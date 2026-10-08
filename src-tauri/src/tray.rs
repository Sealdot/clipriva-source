use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    commands::restore_clipboard_item, reveal_main_window, state::AppState,
    toggle_quick_paste_window,
};

const REVEAL_MAIN_MENU_ID: &str = "reveal-main";
const QUICK_PASTE_MENU_ID: &str = "quick-paste";
const PAUSE_15_MINUTES_MENU_ID: &str = "pause-15-minutes";
const PAUSE_ONE_HOUR_MENU_ID: &str = "pause-one-hour";
const PAUSE_UNTIL_RESUMED_MENU_ID: &str = "pause-until-resumed";
const RESUME_CAPTURE_MENU_ID: &str = "resume-capture";
const LAUNCH_AT_LOGIN_MENU_ID: &str = "launch-at-login";
const QUIT_MENU_ID: &str = "quit";
const RECENT_COPY_MENU_PREFIX: &str = "recent-copy:";
const RECENT_ITEM_LIMIT: u32 = 5;

/// The operating-system menu is a second, deliberately narrow daily-entry
/// surface. It has no network ability: recent actions use the same local
/// clipboard restoration path as the main workspace.
pub struct TrayMenuState {
    icon: TrayIcon<Wry>,
    pause_revision: Arc<AtomicU64>,
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut tray = TrayIconBuilder::with_id("clipriva-tray")
        .tooltip("ClipRiva · local clipboard")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let event_id = event.id().as_ref();
            if let Some(clipboard_item_id) = event_id.strip_prefix(RECENT_COPY_MENU_PREFIX) {
                copy_recent_item(app, clipboard_item_id);
                refresh_menu(app);
                return;
            }

            match event_id {
                REVEAL_MAIN_MENU_ID => reveal_main_window(app),
                QUICK_PASTE_MENU_ID => toggle_quick_paste_window(app),
                PAUSE_15_MINUTES_MENU_ID => pause_capture_for(app, Duration::from_secs(15 * 60)),
                PAUSE_ONE_HOUR_MENU_ID => pause_capture_for(app, Duration::from_secs(60 * 60)),
                PAUSE_UNTIL_RESUMED_MENU_ID => pause_capture_until_resumed(app),
                RESUME_CAPTURE_MENU_ID => resume_capture(app),
                LAUNCH_AT_LOGIN_MENU_ID => toggle_launch_at_login(app),
                QUIT_MENU_ID => app.exit(0),
                _ => {}
            }
            refresh_menu(app);
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left | MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                // Refresh just before the next menu presentation so the five
                // rows always reflect the current local history rather than a
                // startup snapshot.
                refresh_menu(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    let icon = tray.build(app)?;
    app.manage(TrayMenuState {
        icon,
        pause_revision: Arc::new(AtomicU64::new(0)),
    });
    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let preferences = state.database.capture_preferences().ok();
    let is_paused = preferences
        .as_ref()
        .is_some_and(|value| value.capture_paused);
    let status_label = if is_paused {
        format!(
            "Capture paused{}",
            preferences
                .as_ref()
                .and_then(|value| value.pause_reason.as_deref())
                .map(|reason| format!(" · {reason}"))
                .unwrap_or_default(),
        )
    } else {
        "Capture active · local only".to_owned()
    };
    let status = MenuItem::with_id(app, "capture-status", status_label, false, None::<&str>)?;
    let recent_heading =
        MenuItem::with_id(app, "recent-heading", "Recent clips", false, None::<&str>)?;
    let recent_items = state
        .database
        .list("", false, RECENT_ITEM_LIMIT, 0)
        .unwrap_or_default();
    let menu = Menu::new(app)?;
    menu.append(&status)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&recent_heading)?;
    if recent_items.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "recent-empty",
            "Copy something to start a local history",
            false,
            None::<&str>,
        )?)?;
    } else {
        for (index, item) in recent_items.iter().enumerate() {
            let id = format!("{RECENT_COPY_MENU_PREFIX}{}", item.id);
            let label = recent_item_label(index, &item.content, item.source_app.as_deref());
            menu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        QUICK_PASTE_MENU_ID,
        "Open Quick Paste",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        REVEAL_MAIN_MENU_ID,
        "Open main window",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    if is_paused {
        menu.append(&MenuItem::with_id(
            app,
            RESUME_CAPTURE_MENU_ID,
            "Resume capture",
            true,
            None::<&str>,
        )?)?;
    } else {
        menu.append(&MenuItem::with_id(
            app,
            PAUSE_15_MINUTES_MENU_ID,
            "Pause capture for 15 minutes",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            PAUSE_ONE_HOUR_MENU_ID,
            "Pause capture for 1 hour",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            PAUSE_UNTIL_RESUMED_MENU_ID,
            "Pause capture until I resume it",
            true,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        LAUNCH_AT_LOGIN_MENU_ID,
        "Launch at Login",
        true,
        launch_at_login_enabled(app),
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        QUIT_MENU_ID,
        "Quit ClipRiva",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

pub(crate) fn refresh_menu(app: &AppHandle) {
    let Some(tray) = app.try_state::<TrayMenuState>() else {
        return;
    };
    if let Ok(menu) = build_menu(app) {
        let _ = tray.icon.set_menu(Some(menu));
    }
}

fn copy_recent_item(app: &AppHandle, clipboard_item_id: &str) {
    let state = app.state::<AppState>();
    if restore_clipboard_item(app, &state, clipboard_item_id).is_err() {
        // A menu item can become stale if History was cleaned while it was
        // open. Do not place content or database details in diagnostics.
        eprintln!("ClipRiva could not restore the selected recent item.");
    }
}

fn pause_capture_for(app: &AppHandle, duration: Duration) {
    let Some(tray) = app.try_state::<TrayMenuState>() else {
        return;
    };
    let revision = tray.pause_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let label = if duration == Duration::from_secs(15 * 60) {
        "15 minutes"
    } else {
        "1 hour"
    };
    if !set_capture_paused(app, format!("Paused from the menu bar for {label}.")) {
        return;
    }
    let app = app.clone();
    let pause_revision = Arc::clone(&tray.pause_revision);
    thread::spawn(move || {
        thread::sleep(duration);
        if pause_revision.load(Ordering::SeqCst) != revision {
            return;
        }
        let state = app.state::<AppState>();
        let should_resume = state
            .database
            .capture_preferences()
            .ok()
            .is_some_and(|preferences| {
                preferences.capture_paused
                    && preferences.pause_reason.as_deref()
                        == Some(if duration == Duration::from_secs(15 * 60) {
                            "Paused from the menu bar for 15 minutes."
                        } else {
                            "Paused from the menu bar for 1 hour."
                        })
            });
        if should_resume {
            let _ = state.database.resume_capture();
            refresh_menu(&app);
        }
    });
}

fn pause_capture_until_resumed(app: &AppHandle) {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        tray.pause_revision.fetch_add(1, Ordering::SeqCst);
    }
    let _ = set_capture_paused(
        app,
        "Paused from the menu bar until you resume it.".to_owned(),
    );
}

fn set_capture_paused(app: &AppHandle, reason: String) -> bool {
    let state = app.state::<AppState>();
    let Ok(mut preferences) = state.database.capture_preferences() else {
        return false;
    };
    preferences.capture_paused = true;
    preferences.pause_reason = Some(reason);
    state
        .database
        .update_capture_preferences(&preferences)
        .is_ok()
}

fn resume_capture(app: &AppHandle) {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        tray.pause_revision.fetch_add(1, Ordering::SeqCst);
    }
    let state = app.state::<AppState>();
    let _ = state.database.resume_capture();
}

fn recent_item_label(index: usize, content: &str, source_app: Option<&str>) -> String {
    let summary = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let summary = if summary.is_empty() {
        "Untitled clipboard item".to_owned()
    } else if summary.chars().count() > 52 {
        format!("{}…", summary.chars().take(51).collect::<String>())
    } else {
        summary
    };
    let source = source_app
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" · {value}"))
        .unwrap_or_default();
    format!("{}. {summary}{source}", index + 1)
}

fn launch_at_login_enabled(app: &AppHandle) -> bool {
    match app.autolaunch().is_enabled() {
        Ok(enabled) => enabled,
        Err(error) => {
            eprintln!("ClipRiva could not read launch-at-login status: {error}");
            false
        }
    }
}

fn toggle_launch_at_login(app: &AppHandle) {
    match app.autolaunch().is_enabled() {
        Ok(true) => {
            if let Err(error) = app.autolaunch().disable() {
                eprintln!("ClipRiva could not disable launch at login: {error}");
            }
        }
        Ok(false) => {
            if let Err(error) = app.autolaunch().enable() {
                eprintln!("ClipRiva could not enable launch at login: {error}");
            }
        }
        Err(error) => {
            eprintln!("ClipRiva could not change launch-at-login status: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::recent_item_label;

    #[test]
    fn recent_item_labels_are_single_line_and_bounded() {
        let label = recent_item_label(
            0,
            "first line\nsecond line with a deliberately long sequence of words that must be shortened",
            Some("Safari"),
        );

        assert!(label.starts_with("1. first line second line"));
        assert!(label.ends_with("… · Safari"));
        assert!(!label.contains('\n'));
    }

    #[test]
    fn recent_item_labels_do_not_require_source_metadata() {
        assert_eq!(
            recent_item_label(1, "  \n", None),
            "2. Untitled clipboard item"
        );
    }
}
