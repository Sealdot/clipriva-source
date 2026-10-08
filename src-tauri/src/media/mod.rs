//! Local, content-addressed storage for non-text clipboard representations.
//!
//! Clipboard history keeps a short textual preview in SQLite so search and the
//! quick-paste window stay fast. The original bytes for images, rich text and
//! file payloads belong here instead of in SQLite. Database rows should keep
//! only a [`StoredBlob`] reference and may safely share the same blob when the
//! bytes are identical.

use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The maximum representation stored automatically for one clipboard item.
///
/// This keeps an accidental large-file copy from filling the app data
/// directory. A future explicit import flow can opt into larger files.
pub const MAX_AUTOMATIC_BLOB_BYTES: usize = 32 * 1024 * 1024;

/// The representation carried by a clipboard record, distinct from the text
/// preview used for searching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobKind {
    Image,
    RichText,
    File,
}

impl BlobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::RichText => "richText",
            Self::File => "file",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "image" => Some(Self::Image),
            "richText" => Some(Self::RichText),
            "file" => Some(Self::File),
            _ => None,
        }
    }
}

/// Pixel dimensions required when the platform clipboard provides raw RGBA
/// bytes rather than an encoded image. Tauri's desktop clipboard adapter uses
/// this representation, so no image encoder is required on the capture path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

/// Bytes supplied by a platform clipboard adapter or a future explicit import
/// command. `display_name` is presentation metadata only and is never used to
/// build a filesystem path.
#[derive(Debug, Clone)]
pub struct BlobInput<'a> {
    pub kind: BlobKind,
    pub mime_type: &'a str,
    pub display_name: Option<&'a str>,
    pub image_dimensions: Option<ImageDimensions>,
    pub bytes: &'a [u8],
}

/// Serializable metadata to persist alongside a clipboard item.
///
/// `storage_key` is intentionally relative and content-addressed. It can be
/// passed back to [`BlobStore::read`] without exposing the user's app-data
/// directory to the webview or to a future sync layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredBlob {
    pub storage_key: String,
    pub content_hash: String,
    pub kind: BlobKind,
    pub mime_type: String,
    pub display_name: Option<String>,
    pub image_dimensions: Option<ImageDimensions>,
    pub byte_size: u64,
}

#[derive(Debug)]
pub enum BlobStorageError {
    Io(io::Error),
    InvalidInput(&'static str),
    TooLarge { actual: u64, maximum: u64 },
    MissingBlob,
    UnexpectedFileType,
    IntegrityMismatch,
    UnsafeMetadataChange,
}

impl fmt::Display for BlobStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("local clipboard representation storage failed"),
            Self::InvalidInput(message) => {
                write!(formatter, "invalid clipboard representation: {message}")
            }
            Self::TooLarge { actual, maximum } => write!(
                formatter,
                "clipboard representation is {actual} bytes; automatic storage limit is {maximum} bytes"
            ),
            Self::MissingBlob => {
                formatter.write_str("local clipboard representation is unavailable")
            }
            Self::UnexpectedFileType => {
                formatter.write_str("local clipboard representation has an unsafe file type")
            }
            Self::IntegrityMismatch => {
                formatter.write_str("local clipboard representation failed its integrity check")
            }
            Self::UnsafeMetadataChange => formatter
                .write_str("local clipboard representation changed during its safety check"),
        }
    }
}

impl std::error::Error for BlobStorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for BlobStorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub type BlobStorageResult<T> = Result<T, BlobStorageError>;

/// A private local blob directory. All paths are generated by this type; raw
/// clipboard filenames and file URLs are never interpreted as paths.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn open(root: impl Into<PathBuf>) -> BlobStorageResult<Self> {
        let root = root.into();
        ensure_private_directory(&root)?;
        let root = fs::canonicalize(root)?;
        ensure_private_directory(&root.join("sha256"))?;
        Ok(Self { root })
    }

    pub fn store(&self, input: BlobInput<'_>) -> BlobStorageResult<StoredBlob> {
        validate_input(&input)?;

        let content_hash = format!("{:x}", Sha256::digest(input.bytes));
        let storage_key = storage_key_for_hash(&content_hash);
        let path = self.path_for_key(&storage_key)?;

        match self.read_verified(&path, &content_hash) {
            Ok(_) => {}
            Err(BlobStorageError::MissingBlob) => {
                self.write_atomically(&path, &content_hash, input.bytes)?;
            }
            Err(error) => return Err(error),
        }

        Ok(StoredBlob {
            storage_key,
            content_hash,
            kind: input.kind,
            mime_type: input.mime_type.trim().to_owned(),
            display_name: input.display_name.and_then(sanitize_display_name),
            image_dimensions: input.image_dimensions,
            byte_size: input.bytes.len() as u64,
        })
    }

    pub fn read(&self, storage_key: &str) -> BlobStorageResult<Vec<u8>> {
        let expected_hash = hash_for_storage_key(storage_key)?;
        let path = self.path_for_key(storage_key)?;
        self.read_verified(&path, expected_hash)
    }

    #[cfg(test)]
    pub fn contains(&self, storage_key: &str) -> BlobStorageResult<bool> {
        let expected_hash = hash_for_storage_key(storage_key)?;
        let path = self.path_for_key(storage_key)?;
        match self.read_verified(&path, expected_hash) {
            Ok(_) => Ok(true),
            Err(BlobStorageError::MissingBlob) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Removes one content-addressed blob after its last database reference has
    /// been deleted. Reference counting belongs to the SQLite transaction;
    /// this method deliberately does not infer liveness from filenames.
    pub fn remove_unreferenced(&self, storage_key: &str) -> BlobStorageResult<bool> {
        let path = self.path_for_key(storage_key)?;
        self.verify_storage_directories(hash_for_storage_key(storage_key)?)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(BlobStorageError::UnexpectedFileType);
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn path_for_key(&self, storage_key: &str) -> BlobStorageResult<PathBuf> {
        let hash = hash_for_storage_key(storage_key)?;
        Ok(self.root.join("sha256").join(&hash[..2]).join(hash))
    }

    fn read_verified(&self, path: &Path, expected_hash: &str) -> BlobStorageResult<Vec<u8>> {
        self.verify_storage_directories(expected_hash)?;
        let path_metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(BlobStorageError::MissingBlob);
            }
            Err(error) => return Err(error.into()),
        };
        if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
            return Err(BlobStorageError::UnexpectedFileType);
        }

        let file = OpenOptions::new().read(true).open(path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                BlobStorageError::UnsafeMetadataChange
            } else {
                BlobStorageError::Io(error)
            }
        })?;
        let opened_metadata = file.metadata()?;
        if !opened_metadata.is_file() {
            return Err(BlobStorageError::UnexpectedFileType);
        }
        ensure_same_file_identity(&path_metadata, &opened_metadata)?;
        ensure_private_file(&file, &opened_metadata)?;

        let maximum = MAX_AUTOMATIC_BLOB_BYTES as u64;
        if opened_metadata.len() > maximum {
            return Err(BlobStorageError::TooLarge {
                actual: opened_metadata.len(),
                maximum,
            });
        }

        let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
        file.take(maximum + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > maximum {
            return Err(BlobStorageError::TooLarge {
                actual: bytes.len() as u64,
                maximum,
            });
        }
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));
        if actual_hash != expected_hash {
            return Err(BlobStorageError::IntegrityMismatch);
        }

        Ok(bytes)
    }

    fn verify_storage_directories(&self, hash: &str) -> BlobStorageResult<()> {
        verify_private_directory(&self.root)?;
        verify_private_directory(&self.root.join("sha256"))?;
        verify_private_directory(&self.root.join("sha256").join(&hash[..2]))
    }

    fn write_atomically(
        &self,
        destination: &Path,
        expected_hash: &str,
        bytes: &[u8],
    ) -> BlobStorageResult<()> {
        let Some(parent) = destination.parent() else {
            return Err(BlobStorageError::InvalidInput("invalid blob destination"));
        };
        ensure_private_directory(&self.root)?;
        ensure_private_directory(&self.root.join("sha256"))?;
        ensure_private_directory(parent)?;

        let temporary = parent.join(format!(".{}.{}.tmp", Uuid::new_v4(), std::process::id()));
        let write_result = (|| -> io::Result<()> {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(&temporary)?;
            set_private_file_permissions(&file)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            fs::hard_link(&temporary, destination)?;
            fs::remove_file(&temporary)?;
            sync_directory(parent)
        })();

        match write_result {
            Ok(()) => Ok(()),
            // Another capture may have stored the same hash after our
            // initial verification. Never overwrite it; validate it using
            // the same regular-file, size and digest checks as a read.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary);
                self.read_verified(destination, expected_hash).map(|_| ())
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(error.into())
            }
        }
    }
}

/// Creates or repairs one app-owned private directory without changing any
/// parent directory. Existing symlinks and non-directories fail closed.
pub(crate) fn ensure_private_directory(path: &Path) -> BlobStorageResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure_directory_type_and_permissions(path, &metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
            let metadata = fs::symlink_metadata(path)?;
            ensure_directory_type_and_permissions(path, &metadata)
        }
        Err(error) => Err(error.into()),
    }
}

fn verify_private_directory(path: &Path) -> BlobStorageResult<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(BlobStorageError::MissingBlob);
        }
        Err(error) => return Err(error.into()),
    };
    ensure_directory_type_and_permissions(path, &metadata)
}

fn ensure_directory_type_and_permissions(
    path: &Path,
    metadata: &Metadata,
) -> BlobStorageResult<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BlobStorageError::UnexpectedFileType);
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o777 != 0o700 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn ensure_private_file(file: &File, metadata: &Metadata) -> BlobStorageResult<()> {
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o777 != 0o600 {
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn set_private_file_permissions(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn ensure_same_file_identity(before: &Metadata, opened: &Metadata) -> BlobStorageResult<()> {
    #[cfg(unix)]
    if before.dev() != opened.dev() || before.ino() != opened.ino() {
        return Err(BlobStorageError::UnsafeMetadataChange);
    }
    Ok(())
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    Ok(())
}

fn validate_input(input: &BlobInput<'_>) -> BlobStorageResult<()> {
    if input.bytes.is_empty() {
        return Err(BlobStorageError::InvalidInput("empty payload"));
    }
    if input.bytes.len() > MAX_AUTOMATIC_BLOB_BYTES {
        return Err(BlobStorageError::TooLarge {
            actual: input.bytes.len() as u64,
            maximum: MAX_AUTOMATIC_BLOB_BYTES as u64,
        });
    }

    let mime_type = input.mime_type.trim().to_ascii_lowercase();
    if mime_type.is_empty() || mime_type.contains(['\r', '\n']) {
        return Err(BlobStorageError::InvalidInput("invalid MIME type"));
    }
    let mime_essence = mime_type.split(';').next().unwrap_or_default().trim();

    let valid_kind_and_mime = match input.kind {
        BlobKind::Image => {
            if mime_essence == "image/x-clipriva-rgba" {
                matches_raw_rgba(input)
            } else {
                mime_essence.starts_with("image/")
            }
        }
        BlobKind::RichText => matches!(mime_essence, "text/html" | "text/rtf" | "application/rtf"),
        // File payloads may be any declared content type; the caller still
        // needs to attach a display name before the item can be shown as a
        // user-facing file reference.
        BlobKind::File => input.display_name.and_then(sanitize_display_name).is_some(),
    };

    if valid_kind_and_mime {
        Ok(())
    } else {
        Err(BlobStorageError::InvalidInput(
            "representation kind does not match its MIME type or metadata",
        ))
    }
}

fn matches_raw_rgba(input: &BlobInput<'_>) -> bool {
    let Some(dimensions) = input.image_dimensions else {
        return false;
    };
    if dimensions.width == 0 || dimensions.height == 0 {
        return false;
    }

    let Some(expected_bytes) = u64::from(dimensions.width)
        .checked_mul(u64::from(dimensions.height))
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    expected_bytes == input.bytes.len() as u64
}

fn sanitize_display_name(value: &str) -> Option<String> {
    let sanitized = value
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(255)
        .collect::<String>();
    (!sanitized.is_empty()).then_some(sanitized)
}

fn storage_key_for_hash(hash: &str) -> String {
    format!("sha256/{hash}")
}

fn hash_for_storage_key(storage_key: &str) -> BlobStorageResult<&str> {
    let Some((prefix, hash)) = storage_key.split_once('/') else {
        return Err(BlobStorageError::InvalidInput("invalid blob storage key"));
    };
    if prefix != "sha256" || !is_sha256_hash(hash) {
        return Err(BlobStorageError::InvalidInput("invalid blob storage key"));
    }
    Ok(hash)
}

fn is_sha256_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|character| character.is_ascii_digit() || (b'a'..=b'f').contains(&character))
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[cfg(unix)]
    use std::os::unix::fs::{symlink, PermissionsExt};
    #[cfg(unix)]
    use std::os::unix::net::UnixListener;

    use sha2::Digest;

    use super::{
        ensure_same_file_identity, BlobInput, BlobKind, BlobStorageError, BlobStore,
        ImageDimensions, MAX_AUTOMATIC_BLOB_BYTES,
    };

    fn test_store() -> (BlobStore, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("clipriva-blob-test-{}", uuid::Uuid::new_v4()));
        (BlobStore::open(&root).unwrap(), root)
    }

    fn path_for_stored_blob(root: &std::path::Path, storage_key: &str) -> std::path::PathBuf {
        let hash = storage_key.strip_prefix("sha256/").unwrap();
        root.join("sha256").join(&hash[..2]).join(hash)
    }

    fn store_file_blob(store: &BlobStore, bytes: &[u8]) -> super::StoredBlob {
        store
            .store(BlobInput {
                kind: BlobKind::File,
                mime_type: "application/octet-stream",
                display_name: Some("synthetic.bin"),
                image_dimensions: None,
                bytes,
            })
            .unwrap()
    }

    #[test]
    fn stores_and_deduplicates_content_addressed_images() {
        let (store, root) = test_store();
        let bytes = b"not-a-real-png-but-opaque-image-bytes";
        let first = store
            .store(BlobInput {
                kind: BlobKind::Image,
                mime_type: "image/png",
                display_name: Some("Screenshot.png"),
                image_dimensions: None,
                bytes,
            })
            .unwrap();
        let second = store
            .store(BlobInput {
                kind: BlobKind::Image,
                mime_type: "image/png",
                display_name: Some("Second copy.png"),
                image_dimensions: None,
                bytes,
            })
            .unwrap();

        assert_eq!(first.storage_key, second.storage_key);
        assert_eq!(store.read(&first.storage_key).unwrap(), bytes);
        assert!(store.contains(&first.storage_key).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn supports_rich_text_and_file_reference_payloads() {
        let (store, root) = test_store();
        let rich_text = store
            .store(BlobInput {
                kind: BlobKind::RichText,
                mime_type: "text/html",
                display_name: None,
                image_dimensions: None,
                bytes: b"<p><strong>Launch</strong> notes</p>",
            })
            .unwrap();
        let file = store
            .store(BlobInput {
                kind: BlobKind::File,
                mime_type: "application/pdf",
                display_name: Some("launch-brief.pdf"),
                image_dimensions: None,
                bytes: b"file-reference-or-future-file-bytes",
            })
            .unwrap();

        assert_eq!(rich_text.kind, BlobKind::RichText);
        assert_eq!(file.display_name.as_deref(), Some("launch-brief.pdf"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_or_oversized_automatic_payloads() {
        let (store, root) = test_store();
        let invalid = store.store(BlobInput {
            kind: BlobKind::Image,
            mime_type: "text/plain",
            display_name: None,
            image_dimensions: None,
            bytes: b"not an image",
        });
        assert!(matches!(invalid, Err(BlobStorageError::InvalidInput(_))));

        let oversized = vec![0; MAX_AUTOMATIC_BLOB_BYTES + 1];
        let too_large = store.store(BlobInput {
            kind: BlobKind::File,
            mime_type: "application/octet-stream",
            display_name: Some("large.bin"),
            image_dimensions: None,
            bytes: &oversized,
        });
        assert!(matches!(too_large, Err(BlobStorageError::TooLarge { .. })));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_storage_keys_that_cannot_escape_the_blob_root() {
        let (store, root) = test_store();
        let invalid_keys = [
            "/absolute/path",
            "sha256/../../sensitive",
            "sha256\\abcdef",
            "sha256/not-a-hash",
            "sha256/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "other/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ];
        for storage_key in invalid_keys {
            assert!(matches!(
                store.read(storage_key),
                Err(BlobStorageError::InvalidInput(_))
            ));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fails_closed_when_an_existing_blob_is_corrupted() {
        let (store, root) = test_store();
        let stored = store_file_blob(&store, b"original synthetic bytes");
        let path = path_for_stored_blob(&root, &stored.storage_key);
        fs::write(&path, b"changed synthetic bytes").unwrap();

        assert!(matches!(
            store.read(&stored.storage_key),
            Err(BlobStorageError::IntegrityMismatch)
        ));
        assert!(matches!(
            store.store(BlobInput {
                kind: BlobKind::File,
                mime_type: "application/octet-stream",
                display_name: Some("synthetic.bin"),
                image_dimensions: None,
                bytes: b"original synthetic bytes",
            }),
            Err(BlobStorageError::IntegrityMismatch)
        ));
        assert_eq!(fs::read(path).unwrap(), b"changed synthetic bytes");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_an_oversized_existing_blob_before_reading_it() {
        let (store, root) = test_store();
        let hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let storage_key = format!("sha256/{hash}");
        let path = path_for_stored_blob(&root, &storage_key);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = File::create(&path).unwrap();
        file.set_len(MAX_AUTOMATIC_BLOB_BYTES as u64 + 1).unwrap();

        assert!(matches!(
            store.read(&storage_key),
            Err(BlobStorageError::TooLarge { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_directories_instead_of_reading_or_overwriting_them() {
        let (store, root) = test_store();
        let bytes = b"synthetic directory collision";
        let hash = format!("{:x}", sha2::Sha256::digest(bytes));
        let storage_key = format!("sha256/{hash}");
        let path = path_for_stored_blob(&root, &storage_key);
        fs::create_dir_all(&path).unwrap();

        assert!(matches!(
            store.read(&storage_key),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        assert!(matches!(
            store_file_blob_result(&store, bytes),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        assert!(path.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_and_socket_destinations_without_touching_their_targets() {
        // Unix-domain socket paths are short on macOS, so keep this synthetic
        // root intentionally compact while retaining a random component.
        let random = uuid::Uuid::new_v4().simple().to_string();
        let root = std::path::PathBuf::from("/tmp").join(format!("cr-{}", &random[..8]));
        let store = BlobStore::open(&root).unwrap();
        let symlink_bytes = b"synthetic symlink collision";
        let symlink_hash = format!("{:x}", sha2::Sha256::digest(symlink_bytes));
        let symlink_key = format!("sha256/{symlink_hash}");
        let symlink_path = path_for_stored_blob(&root, &symlink_key);
        fs::create_dir_all(symlink_path.parent().unwrap()).unwrap();
        let target = root.join("synthetic-target");
        fs::write(&target, b"target sentinel").unwrap();
        symlink(&target, &symlink_path).unwrap();

        assert!(matches!(
            store.read(&symlink_key),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        assert!(matches!(
            store_file_blob_result(&store, symlink_bytes),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        assert_eq!(fs::read(&target).unwrap(), b"target sentinel");

        let socket_bytes = b"synthetic socket collision";
        let socket_hash = format!("{:x}", sha2::Sha256::digest(socket_bytes));
        let socket_key = format!("sha256/{socket_hash}");
        let socket_path = path_for_stored_blob(&root, &socket_key);
        fs::create_dir_all(socket_path.parent().unwrap()).unwrap();
        let _listener = UnixListener::bind(&socket_path).unwrap();
        assert!(matches!(
            store.read(&socket_key),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        assert!(matches!(
            store_file_blob_result(&store, socket_bytes),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_blob_root_that_is_a_symlink() {
        let parent =
            std::env::temp_dir().join(format!("clipriva-root-test-{}", uuid::Uuid::new_v4()));
        let target = parent.join("target");
        let link = parent.join("link");
        fs::create_dir_all(&target).unwrap();
        symlink(&target, &link).unwrap();

        assert!(matches!(
            BlobStore::open(&link),
            Err(BlobStorageError::UnexpectedFileType)
        ));
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn detects_when_opened_metadata_identifies_a_different_file() {
        let root =
            std::env::temp_dir().join(format!("clipriva-identity-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = root.join("first");
        let second = root.join("second");
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();

        let before = fs::metadata(first).unwrap();
        let opened = File::open(second).unwrap().metadata().unwrap();
        assert!(matches!(
            ensure_same_file_identity(&before, &opened),
            Err(BlobStorageError::UnsafeMetadataChange)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_writers_publish_one_valid_blob_without_temporary_files() {
        let (store, root) = test_store();
        let store = Arc::new(store);
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store_file_blob(&store, b"shared concurrent bytes")
                })
            })
            .collect::<Vec<_>>();
        let stored = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert!(stored
            .iter()
            .all(|candidate| candidate.storage_key == stored[0].storage_key));
        assert_eq!(
            store.read(&stored[0].storage_key).unwrap(),
            b"shared concurrent bytes"
        );
        let parent = path_for_stored_blob(&root, &stored[0].storage_key)
            .parent()
            .unwrap()
            .to_owned();
        assert!(fs::read_dir(parent).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleans_the_private_temporary_file_when_publication_fails() {
        let (store, root) = test_store();
        let parent = root.join("sha256").join("aa");
        fs::create_dir_all(&parent).unwrap();
        // A component longer than NAME_MAX lets the private temporary file be
        // created and synced before the no-clobber hard-link step fails.
        let destination = parent.join("x".repeat(300));
        let result = store.write_atomically(
            &destination,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            b"synthetic publication failure",
        );

        assert!(matches!(result, Err(BlobStorageError::Io(_))));
        assert!(fs::read_dir(parent).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn creates_and_repairs_private_directory_and_file_permissions() {
        let (store, root) = test_store();
        let stored = store_file_blob(&store, b"private synthetic bytes");
        let path = path_for_stored_blob(&root, &stored.storage_key);
        let shard = path.parent().unwrap();

        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join("sha256"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(shard).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(root.join("sha256"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(shard, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let reopened = BlobStore::open(&root).unwrap();
        assert_eq!(
            reopened.read(&stored.storage_key).unwrap(),
            b"private synthetic bytes"
        );
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join("sha256"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(shard).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removes_a_blob_only_when_the_database_has_marked_it_unreferenced() {
        let (store, root) = test_store();
        let stored = store
            .store(BlobInput {
                kind: BlobKind::File,
                mime_type: "application/pdf",
                display_name: Some("brief.pdf"),
                image_dimensions: None,
                bytes: b"file bytes",
            })
            .unwrap();

        assert!(store.remove_unreferenced(&stored.storage_key).unwrap());
        assert!(!store.contains(&stored.storage_key).unwrap());
        assert!(!store.remove_unreferenced(&stored.storage_key).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retains_the_dimensions_needed_to_restore_raw_clipboard_images() {
        let (store, root) = test_store();
        let dimensions = ImageDimensions {
            width: 2,
            height: 1,
        };
        let stored = store
            .store(BlobInput {
                kind: BlobKind::Image,
                mime_type: "image/x-clipriva-rgba",
                display_name: None,
                image_dimensions: Some(dimensions),
                bytes: &[255, 0, 0, 255, 0, 255, 0, 255],
            })
            .unwrap();

        assert_eq!(stored.image_dimensions, Some(dimensions));
        fs::remove_dir_all(root).unwrap();
    }

    fn store_file_blob_result(
        store: &BlobStore,
        bytes: &[u8],
    ) -> super::BlobStorageResult<super::StoredBlob> {
        store.store(BlobInput {
            kind: BlobKind::File,
            mime_type: "application/octet-stream",
            display_name: Some("synthetic.bin"),
            image_dimensions: None,
            bytes,
        })
    }
}
