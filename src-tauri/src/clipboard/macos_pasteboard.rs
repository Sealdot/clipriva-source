use std::path::Path;

use objc2::{runtime::ProtocolObject, ClassType};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardType, NSPasteboardTypeHTML, NSPasteboardTypeRTF,
    NSPasteboardTypeString, NSPasteboardURLReadingFileURLsOnlyKey, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSNumber, NSString, NSURL};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichTextRepresentation {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialClipboardContent {
    RichText {
        plain_text: String,
        representations: Vec<RichTextRepresentation>,
    },
    Files {
        paths: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteboardSnapshot {
    pub change_count: i64,
    /// A random, content-free Local Link ownership marker. The monitor uses it
    /// to consume the Copy event exactly once without saving the received text
    /// into History, including after crash recovery.
    pub local_link_effect: bool,
    pub content: Option<SpecialClipboardContent>,
}

pub const LOCAL_LINK_EFFECT_PASTEBOARD_TYPE: &str = "com.clipriva.desktop.local-link-effect";

pub fn change_count() -> i64 {
    NSPasteboard::generalPasteboard().changeCount() as i64
}

pub fn snapshot_if_changed(previous_change_count: i64) -> Option<PasteboardSnapshot> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount() as i64;
    if change_count == previous_change_count {
        return None;
    }
    let marker_type = NSString::from_str(LOCAL_LINK_EFFECT_PASTEBOARD_TYPE);
    let local_link_effect = pasteboard.stringForType(&marker_type).is_some();

    let files = read_file_paths(&pasteboard);
    if !files.is_empty() {
        return Some(PasteboardSnapshot {
            change_count,
            local_link_effect,
            content: Some(SpecialClipboardContent::Files { paths: files }),
        });
    }

    let plain_text =
        unsafe { pasteboard.stringForType(NSPasteboardTypeString) }.map(|value| value.to_string());
    let mut representations = Vec::new();
    if let Some(data) = unsafe { pasteboard.dataForType(NSPasteboardTypeRTF) } {
        if !data.is_empty() {
            representations.push(RichTextRepresentation {
                mime_type: "text/rtf".to_owned(),
                bytes: data.to_vec(),
            });
        }
    }
    if let Some(data) = unsafe { pasteboard.dataForType(NSPasteboardTypeHTML) } {
        if !data.is_empty() {
            representations.push(RichTextRepresentation {
                mime_type: "text/html".to_owned(),
                bytes: data.to_vec(),
            });
        }
    }

    let content = match (plain_text, representations.is_empty()) {
        (Some(plain_text), false) if !plain_text.trim().is_empty() => {
            Some(SpecialClipboardContent::RichText {
                plain_text,
                representations,
            })
        }
        _ => None,
    };
    Some(PasteboardSnapshot {
        change_count,
        local_link_effect,
        content,
    })
}

pub fn write_rich_text(
    plain_text: &str,
    representations: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let mut types: Vec<&NSPasteboardType> = vec![unsafe { NSPasteboardTypeString }];
    for (mime_type, _) in representations {
        if let Some(pasteboard_type) = pasteboard_type_for_mime(mime_type) {
            if !types.contains(&pasteboard_type) {
                types.push(pasteboard_type);
            }
        }
    }
    let types = NSArray::from_slice(&types);
    unsafe {
        pasteboard.declareTypes_owner(&types, None);
    }

    let plain_text = NSString::from_str(plain_text);
    if !unsafe { pasteboard.setString_forType(&plain_text, NSPasteboardTypeString) } {
        return Err("macOS rejected the plain-text clipboard representation.".to_owned());
    }

    for (mime_type, bytes) in representations {
        let Some(pasteboard_type) = pasteboard_type_for_mime(mime_type) else {
            continue;
        };
        let data = NSData::with_bytes(bytes);
        if !pasteboard.setData_forType(Some(&data), pasteboard_type) {
            return Err(format!(
                "macOS rejected the {mime_type} clipboard representation."
            ));
        }
    }
    Ok(())
}

pub fn write_file_paths(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No local file references are available for this clip.".to_owned());
    }

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    let file_urls = paths
        .iter()
        .filter(|path| Path::new(path).is_absolute())
        .map(|path| {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            ProtocolObject::<dyn NSPasteboardWriting>::from_retained(url)
        })
        .collect::<Vec<_>>();
    if file_urls.is_empty() {
        return Err("No valid local file references are available for this clip.".to_owned());
    }
    let file_urls = NSArray::from_retained_slice(&file_urls);
    if !pasteboard.writeObjects(&file_urls) {
        return Err("macOS rejected the local file references.".to_owned());
    }
    Ok(())
}

fn read_file_paths(pasteboard: &NSPasteboard) -> Vec<String> {
    let classes = NSArray::from_slice(&[NSURL::class()]);
    let options = NSDictionary::from_slices(
        &[unsafe { NSPasteboardURLReadingFileURLsOnlyKey }],
        &[NSNumber::new_bool(true).as_ref()],
    );
    let Some(objects) =
        (unsafe { pasteboard.readObjectsForClasses_options(&classes, Some(&options)) })
    else {
        return Vec::new();
    };

    objects
        .into_iter()
        .filter_map(|value| value.downcast::<NSURL>().ok())
        .filter(|url| url.isFileURL())
        .filter_map(|url| url.path())
        .map(|path| path.to_string())
        .filter(|path| Path::new(path).is_absolute())
        .collect()
}

fn pasteboard_type_for_mime(mime_type: &str) -> Option<&'static NSPasteboardType> {
    match mime_type.split(';').next().unwrap_or_default().trim() {
        "text/rtf" | "application/rtf" => Some(unsafe { NSPasteboardTypeRTF }),
        "text/html" => Some(unsafe { NSPasteboardTypeHTML }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::pasteboard_type_for_mime;

    #[test]
    fn recognizes_only_supported_rich_text_representations() {
        assert!(pasteboard_type_for_mime("text/rtf").is_some());
        assert!(pasteboard_type_for_mime("text/html; charset=utf-8").is_some());
        assert!(pasteboard_type_for_mime("application/json").is_none());
    }
}
