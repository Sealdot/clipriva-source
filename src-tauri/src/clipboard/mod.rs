mod classifier;
pub(crate) mod foreground_app;
pub mod image_capture;
#[cfg(target_os = "macos")]
pub(crate) mod macos_pasteboard;
pub mod monitor;
pub mod policy;

pub use classifier::{classify_text, max_bytes_for_kind, normalize_text_for_storage};
