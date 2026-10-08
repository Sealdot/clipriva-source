use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::db::Database;

use super::image_capture::{read_system_image, ImageCaptureError};

use super::foreground_app::{ForegroundApplicationProvider, PlatformForegroundApplicationProvider};

const POLL_INTERVAL: Duration = Duration::from_millis(650);

pub fn start(app: AppHandle, database: Arc<Database>) {
    tauri::async_runtime::spawn(async move {
        let mut previous_text = String::new();
        let mut previous_image_hash = String::new();
        #[cfg(target_os = "macos")]
        let mut previous_change_count = -1;
        let foreground_app_provider = PlatformForegroundApplicationProvider;

        loop {
            #[cfg(target_os = "macos")]
            match capture_special_content_if_changed(
                &app,
                &mut previous_change_count,
                &mut previous_text,
                &mut previous_image_hash,
                &database,
                &foreground_app_provider,
            ) {
                SpecialCaptureDecision::Unchanged => {
                    tokio::time::sleep(POLL_INTERVAL).await;
                    continue;
                }
                SpecialCaptureDecision::Captured => {
                    tokio::time::sleep(POLL_INTERVAL).await;
                    continue;
                }
                SpecialCaptureDecision::ReadStandardContent => {}
            }

            match app.clipboard().read_text() {
                Ok(content) if !content.trim().is_empty() => {
                    if let Some(item_id) = capture_if_changed(
                        &content,
                        &mut previous_text,
                        &database,
                        &foreground_app_provider,
                    ) {
                        enqueue_capture_if_collecting(&app, &item_id);
                        #[cfg(desktop)]
                        crate::tray::refresh_menu(&app);
                    }
                    // A later copy of the same image is a new clipboard event
                    // once text has taken its place.
                    previous_image_hash.clear();
                }
                _ => capture_image_if_changed(
                    &app,
                    &mut previous_text,
                    &mut previous_image_hash,
                    &database,
                    &foreground_app_provider,
                ),
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecialCaptureDecision {
    Unchanged,
    Captured,
    ReadStandardContent,
}

#[cfg(target_os = "macos")]
fn capture_special_content_if_changed<P: ForegroundApplicationProvider>(
    app: &AppHandle,
    previous_change_count: &mut i64,
    previous_text: &mut String,
    previous_image_hash: &mut String,
    database: &Database,
    foreground_app_provider: &P,
) -> SpecialCaptureDecision {
    use super::macos_pasteboard::{snapshot_if_changed, SpecialClipboardContent};

    let Some(snapshot) = snapshot_if_changed(*previous_change_count) else {
        return SpecialCaptureDecision::Unchanged;
    };
    *previous_change_count = snapshot.change_count;
    if snapshot.local_link_effect {
        previous_text.clear();
        previous_image_hash.clear();
        return SpecialCaptureDecision::Captured;
    }
    if app
        .try_state::<crate::quick_paste::QuickPasteState>()
        .is_some_and(|state| state.consume_self_write(snapshot.change_count))
    {
        previous_text.clear();
        previous_image_hash.clear();
        return SpecialCaptureDecision::Captured;
    }

    let Some(content) = snapshot.content else {
        // On macOS, changeCount is the event identity. Clearing the previous
        // text lets a deliberate second copy of the exact same value append a
        // new Occurrence instead of being mistaken for a polling duplicate.
        previous_text.clear();
        return SpecialCaptureDecision::ReadStandardContent;
    };
    let source_app = foreground_app_provider.frontmost_application();
    let result = match content {
        SpecialClipboardContent::RichText {
            plain_text,
            representations,
        } => {
            let representations = representations
                .into_iter()
                .map(|representation| (representation.mime_type, representation.bytes))
                .collect::<Vec<_>>();
            database.capture_rich_text(&plain_text, &representations, source_app.as_deref())
        }
        SpecialClipboardContent::Files { paths } => {
            database.capture_file_references(&paths, source_app.as_deref())
        }
    };
    match result {
        Ok(Some(item)) => {
            enqueue_capture_if_collecting(app, &item.id);
            #[cfg(desktop)]
            crate::tray::refresh_menu(app);
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("ClipRiva could not persist rich clipboard content: {error}");
        }
    }
    previous_text.clear();
    previous_image_hash.clear();
    SpecialCaptureDecision::Captured
}

fn capture_image_if_changed<P: ForegroundApplicationProvider>(
    app: &AppHandle,
    previous_text: &mut String,
    previous_image_hash: &mut String,
    database: &Database,
    foreground_app_provider: &P,
) {
    let image = match read_system_image(app) {
        Ok(image) => image,
        Err(ImageCaptureError::Encoding(_)) => {
            // The clipboard contained an image ClipRiva cannot encode within
            // its safe local limits. Record only the reason and source app;
            // never write image bytes or a preview for this status event.
            let source_app = foreground_app_provider.frontmost_application();
            if let Err(error) = database.record_unsupported_capture_format(source_app.as_deref()) {
                eprintln!("ClipRiva could not record unsupported clipboard format: {error}");
            }
            return;
        }
        // The common path for a non-image clipboard. Clipboard plugin errors
        // here are deliberately non-fatal, like an unavailable source app.
        Err(_) => return,
    };
    let content_hash = image.content_hash();
    if content_hash == *previous_image_hash {
        return;
    }

    previous_image_hash.clear();
    previous_image_hash.push_str(&content_hash);
    previous_text.clear();
    let source_app = foreground_app_provider.frontmost_application();
    match database.capture_image(&image, source_app.as_deref()) {
        Ok(Some(item)) => {
            enqueue_capture_if_collecting(app, &item.id);
            #[cfg(desktop)]
            crate::tray::refresh_menu(app);
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("ClipRiva could not persist clipboard image: {error}");
        }
    }
}

fn capture_if_changed<P: ForegroundApplicationProvider>(
    content: &str,
    previous: &mut String,
    database: &Database,
    foreground_app_provider: &P,
) -> Option<String> {
    if content.trim().is_empty() || content == previous {
        return None;
    }

    previous.clear();
    previous.push_str(content);

    // Attribution is optional metadata. A provider failure returns `None` and
    // capture proceeds with the same reliability as it did before attribution.
    let source_app = foreground_app_provider.frontmost_application();
    match database.capture_text(content, source_app.as_deref()) {
        Ok(Some(item)) => Some(item.id),
        Ok(None) => None,
        Err(error) => {
            eprintln!("ClipRiva could not persist clipboard content: {error}");
            None
        }
    }
}

fn enqueue_capture_if_collecting(app: &AppHandle, item_id: &str) {
    let Some(stack) = app.try_state::<crate::clipboard_stack::ClipboardStack>() else {
        return;
    };
    let Some(result) = stack.enqueue_captured(item_id) else {
        return;
    };
    if result.added_count > 0 {
        let _ = app.emit(
            crate::clipboard_stack::CLIPBOARD_STACK_CHANGED_EVENT,
            &result.snapshot,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{capture_if_changed, ForegroundApplicationProvider};
    use crate::clipboard::policy::{CapturePreferences, SensitiveContentPolicy};
    use crate::db::Database;

    struct TestForegroundApplicationProvider {
        application: Option<String>,
        calls: Cell<u32>,
    }

    impl ForegroundApplicationProvider for TestForegroundApplicationProvider {
        fn frontmost_application(&self) -> Option<String> {
            self.calls.set(self.calls.get() + 1);
            self.application.clone()
        }
    }

    #[test]
    fn captures_changed_content_with_its_source_application() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut previous = String::new();
        let provider = TestForegroundApplicationProvider {
            application: Some("Safari".to_owned()),
            calls: Cell::new(0),
        };

        capture_if_changed("copied from a browser", &mut previous, &database, &provider);

        let item = database.list("", false, 1, 0).unwrap().pop().unwrap();
        assert_eq!(item.source_app.as_deref(), Some("Safari"));
        assert_eq!(provider.calls.get(), 1);
    }

    #[test]
    fn requests_attribution_only_for_new_nonempty_content() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut previous = String::new();
        let provider = TestForegroundApplicationProvider {
            application: None,
            calls: Cell::new(0),
        };

        capture_if_changed("first copy", &mut previous, &database, &provider);
        capture_if_changed("first copy", &mut previous, &database, &provider);
        capture_if_changed(" \n\t", &mut previous, &database, &provider);

        assert_eq!(provider.calls.get(), 1);
    }

    #[test]
    fn captures_content_when_attribution_is_unavailable() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut previous = String::new();
        let provider = TestForegroundApplicationProvider {
            application: None,
            calls: Cell::new(0),
        };

        capture_if_changed(
            "copied without a source",
            &mut previous,
            &database,
            &provider,
        );

        let item = database.list("", false, 1, 0).unwrap().pop().unwrap();
        assert_eq!(item.source_app, None);
        assert_eq!(provider.calls.get(), 1);
    }

    #[test]
    fn default_sensitive_policy_skips_only_the_current_monitor_event() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut previous = String::new();
        let provider = TestForegroundApplicationProvider {
            application: Some("Terminal".to_owned()),
            calls: Cell::new(0),
        };

        capture_if_changed(
            "-----BEGIN PRIVATE KEY-----",
            &mut previous,
            &database,
            &provider,
        );
        capture_if_changed("safe follow-up note", &mut previous, &database, &provider);

        let items = database.list("", false, 10, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "safe follow-up note");
        assert!(!database.capture_preferences().unwrap().capture_paused);
        assert_eq!(provider.calls.get(), 2);
    }

    #[test]
    fn strict_sensitive_policy_pauses_subsequent_monitor_events() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let preferences = CapturePreferences {
            sensitive_content_policy: SensitiveContentPolicy::Strict,
            sensitive_pause_enabled: true,
            ..CapturePreferences::default()
        };
        database.update_capture_preferences(&preferences).unwrap();
        let mut previous = String::new();
        let provider = TestForegroundApplicationProvider {
            application: Some("Terminal".to_owned()),
            calls: Cell::new(0),
        };

        capture_if_changed(
            "-----BEGIN PRIVATE KEY-----",
            &mut previous,
            &database,
            &provider,
        );
        capture_if_changed("safe follow-up note", &mut previous, &database, &provider);

        assert!(database.list("", false, 10, 0).unwrap().is_empty());
        let preferences = database.capture_preferences().unwrap();
        assert!(preferences.capture_paused);
        assert_eq!(
            preferences.pause_reason.as_deref(),
            Some("Sensitive content detected. Capture is paused until you resume it.")
        );
        assert_eq!(provider.calls.get(), 2);
    }
}
