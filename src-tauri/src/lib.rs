mod actions;
pub mod automation;
mod clipboard;
mod clipboard_stack;
mod commands;
pub mod context;
mod db;
mod error;
mod local_link;
mod macos_ocr;
mod media;
mod models;
mod quick_paste;
mod state;
#[cfg(desktop)]
mod tray;

use std::sync::Arc;

#[cfg(desktop)]
use tauri::Emitter;
use tauri::Manager;
#[cfg(desktop)]
use tauri::{PhysicalPosition, PhysicalSize};
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, GlobalShortcutExt, Shortcut, ShortcutState,
};

use crate::db::Database;
use crate::state::AppState;

#[cfg(desktop)]
pub(crate) fn toggle_quick_paste_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("quick-paste") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    if let Some(state) = app.try_state::<quick_paste::QuickPasteState>() {
        state.remember_frontmost_application();
    }
    fit_quick_paste_to_work_area(&window);
    let _ = window.emit("quick-paste-opened", ());
    let _ = window.show();
    let _ = window.set_focus();
}

#[cfg(desktop)]
fn fit_quick_paste_to_work_area(window: &tauri::WebviewWindow) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let work_area = monitor.work_area();
    let Some((width, height)) = quick_paste_size_for_work_area(
        work_area.size.width,
        work_area.size.height,
        monitor.scale_factor(),
    ) else {
        return;
    };
    let minimum = PhysicalSize::new(
        ((520.0 * monitor.scale_factor()).round() as u32).min(width),
        ((380.0 * monitor.scale_factor()).round() as u32).min(height),
    );
    let _ = window.set_min_size(Some(minimum));
    let _ = window.set_size(PhysicalSize::new(width, height));
    let x = work_area.position.x + ((work_area.size.width - width) / 2) as i32;
    let y = work_area.position.y + ((work_area.size.height - height) / 2) as i32;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(desktop)]
fn quick_paste_size_for_work_area(width: u32, height: u32, scale: f64) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some((
        ((600.0 * scale).round() as u32).min(width),
        ((454.0 * scale).round() as u32).min(height),
    ))
}

#[cfg(all(test, desktop))]
mod quick_paste_window_tests {
    use super::quick_paste_size_for_work_area;

    #[test]
    fn quick_paste_fits_regular_retina_and_small_work_areas() {
        assert_eq!(
            quick_paste_size_for_work_area(1440, 900, 1.0),
            Some((600, 454))
        );
        assert_eq!(
            quick_paste_size_for_work_area(2880, 1800, 2.0),
            Some((1200, 908))
        );
        assert_eq!(
            quick_paste_size_for_work_area(500, 350, 1.0),
            Some((500, 350))
        );
        assert_eq!(quick_paste_size_for_work_area(0, 350, 1.0), None);
    }
}

#[cfg(desktop)]
pub(crate) fn reveal_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    let _ = window.set_focus();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_clipboard_manager::init());
    #[cfg(desktop)]
    let builder = builder
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let stack_pressed = app
                            .try_state::<AppState>()
                            .and_then(|state| state.database.capture_preferences().ok())
                            .and_then(|preferences| {
                                preferences.stack_shortcut.parse::<Shortcut>().ok()
                            })
                            .is_some_and(|configured| configured.id() == shortcut.id());
                        if stack_pressed {
                            reveal_main_window(app);
                            let _ = app.emit_to("main", "clipriva://clipboard-stack-open-v1", ());
                        } else {
                            toggle_quick_paste_window(app);
                        }
                    }
                })
                .build(),
        )
        // Login startup is opt-in and can be changed from the tray menu.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("ClipRiva")
                .build(),
        )
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        });

    let app = builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            media::ensure_private_directory(&data_dir)?;
            let database = Arc::new(Database::open(&data_dir.join("clipriva.sqlite3"))?);
            let automation = automation::runtime::AutomationRuntime::new(
                Arc::clone(&database),
                app.handle().clone(),
                data_dir.join("automation-v1.sock"),
            )?;
            let local_link = local_link::LocalLinkService::new(Arc::clone(&database))?;
            commands::recover_local_link_copy_effects_natively(app.handle(), local_link.as_ref())?;
            local_link.install_platform_lifecycle_monitor();
            #[cfg(desktop)]
            {
                let preferences = database.capture_preferences()?;
                app.global_shortcut().register_multiple([
                    preferences.quick_paste_shortcut.as_str(),
                    preferences.stack_shortcut.as_str(),
                ])?;
            }

            app.manage(quick_paste::QuickPasteState::default());
            app.manage(clipboard_stack::ClipboardStack::new());
            app.manage(automation);
            app.manage(AppState {
                database,
                local_link,
            });
            let database = Arc::clone(&app.state::<AppState>().database);
            clipboard::monitor::start(app.handle().clone(), database);
            #[cfg(desktop)]
            tray::setup(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_clipboard_items,
            commands::get_clipboard_item,
            commands::get_clipboard_filter_options,
            commands::extract_clipboard_image_text,
            commands::cancel_clipboard_image_text_extraction,
            commands::get_clipboard_image_text,
            commands::delete_clipboard_image_text,
            commands::list_clipboard_collections,
            commands::create_clipboard_smart_collection,
            commands::update_clipboard_smart_collection,
            commands::delete_clipboard_smart_collection,
            commands::get_clipboard_item_note,
            commands::save_clipboard_item_note,
            commands::delete_clipboard_item_note,
            commands::search_quick_paste_items,
            commands::search_local_semantic_clipboard_items,
            commands::copy_clipboard_item,
            commands::use_clipboard_item,
            commands::get_paste_permission_status,
            commands::request_paste_permission,
            commands::open_paste_permission_settings,
            commands::toggle_clipboard_pin,
            commands::replace_clipboard_tags,
            commands::save_clipboard_snippet,
            commands::remove_clipboard_snippet,
            commands::rename_clipboard_collection,
            commands::delete_clipboard_collection,
            commands::delete_clipboard_item,
            commands::clear_clipboard_history,
            commands::preview_clipboard_cleanup,
            commands::move_clipboard_items_to_recycle_bin,
            commands::list_recycle_bin_items,
            commands::restore_clipboard_item_from_recycle_bin,
            commands::permanently_delete_recycle_bin_items,
            commands::empty_clipboard_recycle_bin,
            commands::get_capture_preferences,
            commands::update_capture_preferences,
            commands::resume_clipboard_capture,
            commands::list_recent_capture_status_events,
            commands::clear_capture_status_events,
            commands::record_local_diagnostic_event,
            commands::get_local_diagnostics_summary,
            commands::clear_local_diagnostics,
            commands::export_local_diagnostics,
            commands::get_clipboard_enrichment,
            commands::list_local_text_actions,
            commands::create_local_text_action,
            commands::update_local_text_action,
            commands::delete_local_text_action,
            commands::preview_local_text_action,
            commands::confirm_local_text_action,
            commands::list_action_audit,
            commands::preview_cloud_action,
            commands::get_local_link_preferences,
            commands::get_local_link_readiness_snapshot,
            commands::build_local_link_diagnostics,
            commands::get_local_link_identity_fingerprint,
            commands::reset_local_link_identity,
            commands::update_local_link_preferences,
            commands::list_local_link_devices,
            commands::begin_local_link_pairing,
            commands::get_local_link_pairing,
            commands::confirm_local_link_pairing,
            commands::cancel_local_link_pairing,
            commands::revoke_local_link_device,
            commands::send_clipboard_item_to_device,
            commands::list_local_link_transfers,
            commands::clear_local_link_transfers,
            commands::reveal_local_link_transfer,
            commands::mark_local_link_transfer_viewed,
            commands::cancel_local_link_transfer,
            commands::accept_local_link_transfer,
            commands::reject_local_link_transfer,
            commands::retry_local_link_transfer,
            clipboard_stack::get_clipboard_stack,
            clipboard_stack::add_clipboard_items_to_stack,
            clipboard_stack::set_clipboard_stack_collecting,
            clipboard_stack::move_clipboard_stack_item,
            clipboard_stack::remove_clipboard_stack_item,
            clipboard_stack::begin_clipboard_stack_activation,
            clipboard_stack::finish_clipboard_stack_activation,
            clipboard_stack::advance_clipboard_stack,
            clipboard_stack::mark_clipboard_stack_item_unavailable,
            clipboard_stack::reset_clipboard_stack,
            automation::runtime::get_automation_preferences,
            automation::runtime::update_automation_preferences,
        ])
        .build(tauri::generate_context!())
        .expect("error while building ClipRiva");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            reveal_main_window(app);
        }
    });
}

#[cfg(desktop)]
pub(crate) fn replace_global_shortcuts(
    app: &tauri::AppHandle,
    previous: &crate::clipboard::policy::CapturePreferences,
    next: &crate::clipboard::policy::CapturePreferences,
) -> Result<(), String> {
    let previous_shortcuts = [
        previous.quick_paste_shortcut.trim(),
        previous.stack_shortcut.trim(),
    ];
    let next_shortcuts = [next.quick_paste_shortcut.trim(), next.stack_shortcut.trim()];
    if previous_shortcuts == next_shortcuts {
        return Ok(());
    }

    for shortcut in previous_shortcuts {
        let _ = app.global_shortcut().unregister(shortcut);
    }
    if let Err(error) = app.global_shortcut().register_multiple(next_shortcuts) {
        for shortcut in next_shortcuts {
            let _ = app.global_shortcut().unregister(shortcut);
        }
        let _ = app.global_shortcut().register_multiple(previous_shortcuts);
        return Err(format!(
            "Could not register the new global shortcuts: {error}"
        ));
    }
    Ok(())
}

#[cfg(desktop)]
pub(crate) fn restore_global_shortcuts(
    app: &tauri::AppHandle,
    previous: &crate::clipboard::policy::CapturePreferences,
    rejected: &crate::clipboard::policy::CapturePreferences,
) {
    for shortcut in [
        rejected.quick_paste_shortcut.trim(),
        rejected.stack_shortcut.trim(),
    ] {
        let _ = app.global_shortcut().unregister(shortcut);
    }
    let _ = app.global_shortcut().register_multiple([
        previous.quick_paste_shortcut.trim(),
        previous.stack_shortcut.trim(),
    ]);
}
