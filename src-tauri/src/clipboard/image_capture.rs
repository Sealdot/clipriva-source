//! Platform-image capture that stays independent from the database layer.
//!
//! The desktop clipboard plugin returns raw RGBA pixels. This module encodes
//! them as a self-contained PNG before handing them to the local BlobStore, so
//! an image clip can be restored later without preserving a system pasteboard
//! handle or requesting cloud access.

use std::fmt;

use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::media::{
    BlobInput, BlobKind, BlobStorageError, BlobStore, ImageDimensions, StoredBlob,
    MAX_AUTOMATIC_BLOB_BYTES,
};

#[derive(Debug)]
pub enum ImageCaptureError {
    Clipboard(String),
    Encoding(ImageEncodingError),
    Storage(BlobStorageError),
}

impl fmt::Display for ImageCaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clipboard(error) => {
                write!(formatter, "system clipboard image read failed: {error}")
            }
            Self::Encoding(error) => write!(formatter, "clipboard image encoding failed: {error}"),
            Self::Storage(error) => write!(formatter, "clipboard image storage failed: {error}"),
        }
    }
}

impl std::error::Error for ImageCaptureError {}

impl From<ImageEncodingError> for ImageCaptureError {
    fn from(error: ImageEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl From<BlobStorageError> for ImageCaptureError {
    fn from(error: BlobStorageError) -> Self {
        Self::Storage(error)
    }
}

/// A locally encoded clipboard image, ready to be stored after capture policy
/// evaluation. Keeping this separate from storage ensures denied or paused
/// captures never create a blob on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedImage {
    pub png: Vec<u8>,
    pub dimensions: ImageDimensions,
}

impl CapturedImage {
    pub fn content_hash(&self) -> String {
        format!("{:x}", Sha256::digest(&self.png))
    }
}

/// Reads and encodes the current system image without writing it to disk.
pub fn read_system_image(app: &AppHandle) -> Result<CapturedImage, ImageCaptureError> {
    let image = app
        .clipboard()
        .read_image()
        .map_err(|error| ImageCaptureError::Clipboard(error.to_string()))?;
    let dimensions = ImageDimensions {
        width: image.width(),
        height: image.height(),
    };
    let png = encode_rgba_png(image.rgba(), dimensions.width, dimensions.height)?;

    Ok(CapturedImage { png, dimensions })
}

/// Stores a policy-approved encoded image in the private content-addressed
/// blob store.
pub fn store_captured_image(
    blob_store: &BlobStore,
    image: &CapturedImage,
) -> Result<StoredBlob, ImageCaptureError> {
    blob_store
        .store(BlobInput {
            kind: BlobKind::Image,
            mime_type: "image/png",
            display_name: None,
            image_dimensions: Some(image.dimensions),
            bytes: &image.png,
        })
        .map_err(ImageCaptureError::from)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageEncodingError {
    ZeroDimensions,
    InvalidRgbaLength { actual: usize, expected: usize },
    ImageTooLarge,
}

impl fmt::Display for ImageEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimensions => write!(formatter, "image dimensions must be non-zero"),
            Self::InvalidRgbaLength { actual, expected } => {
                write!(
                    formatter,
                    "expected {expected} RGBA bytes but received {actual}"
                )
            }
            Self::ImageTooLarge => {
                write!(formatter, "image dimensions exceed the PNG encoder limit")
            }
        }
    }
}

impl std::error::Error for ImageEncodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageDecodingError {
    InvalidPng(&'static str),
    UnsupportedPng(&'static str),
    CorruptPng(&'static str),
    ImageTooLarge,
}

impl fmt::Display for ImageDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPng(message) => write!(formatter, "invalid ClipRiva PNG: {message}"),
            Self::UnsupportedPng(message) => write!(formatter, "unsupported PNG: {message}"),
            Self::CorruptPng(message) => write!(formatter, "corrupt PNG: {message}"),
            Self::ImageTooLarge => write!(
                formatter,
                "PNG dimensions exceed the automatic capture limit"
            ),
        }
    }
}

impl std::error::Error for ImageDecodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedRgbaImage {
    pub dimensions: ImageDimensions,
    pub pixels: Vec<u8>,
}

/// Encodes a raw RGBA image into a standards-compliant PNG with unfiltered
/// scanlines and lossless DEFLATE stored blocks. It intentionally has no image
/// crate dependency: clipboard capture has a small, deterministic conversion
/// path and the original RGBA dimensions are retained in `StoredBlob`.
pub fn encode_rgba_png(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, ImageEncodingError> {
    if width == 0 || height == 0 {
        return Err(ImageEncodingError::ZeroDimensions);
    }
    let expected_rgba_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(ImageEncodingError::ImageTooLarge)?;
    if rgba.len() != expected_rgba_bytes {
        return Err(ImageEncodingError::InvalidRgbaLength {
            actual: rgba.len(),
            expected: expected_rgba_bytes,
        });
    }

    let row_bytes =
        usize::try_from(u64::from(width) * 4).map_err(|_| ImageEncodingError::ImageTooLarge)?;
    let encoded_row_bytes = row_bytes
        .checked_add(1)
        .ok_or(ImageEncodingError::ImageTooLarge)?;
    let raw_capacity = encoded_row_bytes
        .checked_mul(usize::try_from(height).map_err(|_| ImageEncodingError::ImageTooLarge)?)
        .ok_or(ImageEncodingError::ImageTooLarge)?;
    let zlib_overhead = raw_capacity
        .checked_add(65_534)
        .map(|value| value / 65_535 * 5 + 6)
        .ok_or(ImageEncodingError::ImageTooLarge)?;
    let png_upper_bound = raw_capacity
        .checked_add(zlib_overhead)
        .and_then(|value| value.checked_add(57))
        .ok_or(ImageEncodingError::ImageTooLarge)?;
    if png_upper_bound > MAX_AUTOMATIC_BLOB_BYTES {
        return Err(ImageEncodingError::ImageTooLarge);
    }
    let mut scanlines = Vec::with_capacity(raw_capacity);
    for row in rgba.chunks_exact(row_bytes) {
        scanlines.push(0); // PNG filter: None
        scanlines.extend_from_slice(row);
    }

    let mut png = Vec::with_capacity(8 + 25 + scanlines.len() + 32);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, no interlace
    append_png_chunk(&mut png, *b"IHDR", &ihdr);

    let compressed = encode_zlib_stored(&scanlines);
    append_png_chunk(&mut png, *b"IDAT", &compressed);
    append_png_chunk(&mut png, *b"IEND", &[]);
    Ok(png)
}

/// Decodes the deterministic PNG flavor emitted by [`encode_rgba_png`].
///
/// This is deliberately not a general-purpose PNG decoder. It accepts the
/// non-interlaced, 8-bit RGBA PNG with unfiltered rows and DEFLATE stored
/// blocks that ClipRiva writes, which gives the copy-back command a fully
/// local restoration path without another native image dependency.
pub fn decode_clipriva_rgba_png(png: &[u8]) -> Result<DecodedRgbaImage, ImageDecodingError> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if png.get(..8) != Some(PNG_SIGNATURE) {
        return Err(ImageDecodingError::InvalidPng("missing PNG signature"));
    }

    let mut offset = 8;
    let mut dimensions = None;
    let mut compressed = Vec::new();
    let mut saw_iend = false;
    while offset < png.len() {
        let (kind, data, next_offset) = read_png_chunk(png, offset)?;
        match &kind {
            b"IHDR" => {
                if dimensions.is_some() || data.len() != 13 {
                    return Err(ImageDecodingError::InvalidPng("invalid IHDR"));
                }
                let width = u32::from_be_bytes(
                    data[0..4]
                        .try_into()
                        .map_err(|_| ImageDecodingError::InvalidPng("invalid width"))?,
                );
                let height = u32::from_be_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| ImageDecodingError::InvalidPng("invalid height"))?,
                );
                if width == 0 || height == 0 {
                    return Err(ImageDecodingError::InvalidPng("zero image dimensions"));
                }
                if data[8..] != [8, 6, 0, 0, 0] {
                    return Err(ImageDecodingError::UnsupportedPng(
                        "only 8-bit RGBA, non-interlaced images are supported",
                    ));
                }
                dimensions = Some(ImageDimensions { width, height });
            }
            b"IDAT" => compressed.extend_from_slice(data),
            b"IEND" => {
                if !data.is_empty() || saw_iend || next_offset != png.len() {
                    return Err(ImageDecodingError::InvalidPng("invalid IEND"));
                }
                saw_iend = true;
                break;
            }
            _ => {}
        }
        offset = next_offset;
    }

    let dimensions = dimensions.ok_or(ImageDecodingError::InvalidPng("missing IHDR"))?;
    if !saw_iend {
        return Err(ImageDecodingError::InvalidPng("missing IEND"));
    }
    let scanline_len = scanline_byte_len(dimensions)?;
    let scanlines = decode_zlib_stored(&compressed)?;
    if scanlines.len() != scanline_len {
        return Err(ImageDecodingError::CorruptPng(
            "unexpected decompressed length",
        ));
    }

    let row_bytes = usize::try_from(u64::from(dimensions.width) * 4)
        .map_err(|_| ImageDecodingError::ImageTooLarge)?;
    let mut pixels =
        Vec::with_capacity(scanlines.len() - usize::try_from(dimensions.height).unwrap_or(0));
    for row in scanlines.chunks_exact(row_bytes + 1) {
        if row[0] != 0 {
            return Err(ImageDecodingError::UnsupportedPng(
                "only unfiltered scanlines are supported",
            ));
        }
        pixels.extend_from_slice(&row[1..]);
    }

    Ok(DecodedRgbaImage { dimensions, pixels })
}

fn read_png_chunk(
    png: &[u8],
    offset: usize,
) -> Result<([u8; 4], &[u8], usize), ImageDecodingError> {
    let length_bytes = png
        .get(offset..offset + 4)
        .ok_or(ImageDecodingError::InvalidPng("truncated chunk length"))?;
    let length = u32::from_be_bytes(
        length_bytes
            .try_into()
            .map_err(|_| ImageDecodingError::InvalidPng("invalid chunk length"))?,
    ) as usize;
    let kind_start = offset + 4;
    let kind: [u8; 4] = png
        .get(kind_start..kind_start + 4)
        .ok_or(ImageDecodingError::InvalidPng("truncated chunk type"))?
        .try_into()
        .map_err(|_| ImageDecodingError::InvalidPng("invalid chunk type"))?;
    let data_start = kind_start + 4;
    let data_end = data_start
        .checked_add(length)
        .ok_or(ImageDecodingError::InvalidPng("chunk length overflow"))?;
    let next_offset = data_end
        .checked_add(4)
        .ok_or(ImageDecodingError::InvalidPng("chunk length overflow"))?;
    let data = png
        .get(data_start..data_end)
        .ok_or(ImageDecodingError::InvalidPng("truncated chunk data"))?;
    let checksum = png
        .get(data_end..next_offset)
        .ok_or(ImageDecodingError::InvalidPng("truncated chunk checksum"))?;
    let checksum = u32::from_be_bytes(
        checksum
            .try_into()
            .map_err(|_| ImageDecodingError::InvalidPng("invalid chunk checksum"))?,
    );
    if crc32(&kind, data) != checksum {
        return Err(ImageDecodingError::CorruptPng("chunk checksum mismatch"));
    }
    Ok((kind, data, next_offset))
}

fn scanline_byte_len(dimensions: ImageDimensions) -> Result<usize, ImageDecodingError> {
    let row_bytes = u64::from(dimensions.width)
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or(ImageDecodingError::ImageTooLarge)?;
    let bytes = row_bytes
        .checked_mul(u64::from(dimensions.height))
        .ok_or(ImageDecodingError::ImageTooLarge)?;
    let bytes = usize::try_from(bytes).map_err(|_| ImageDecodingError::ImageTooLarge)?;
    let zlib_overhead = bytes
        .checked_add(65_534)
        .map(|value| value / 65_535 * 5 + 6)
        .ok_or(ImageDecodingError::ImageTooLarge)?;
    let png_upper_bound = bytes
        .checked_add(zlib_overhead)
        .and_then(|value| value.checked_add(57))
        .ok_or(ImageDecodingError::ImageTooLarge)?;
    if png_upper_bound > MAX_AUTOMATIC_BLOB_BYTES {
        return Err(ImageDecodingError::ImageTooLarge);
    }
    Ok(bytes)
}

fn append_png_chunk(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(&kind);
    output.extend_from_slice(data);
    output.extend_from_slice(&crc32(&kind, data).to_be_bytes());
}

fn encode_zlib_stored(input: &[u8]) -> Vec<u8> {
    // 0x78 0x01 is a valid zlib header for no-compression/fastest mode.
    let mut output = Vec::with_capacity(input.len() + input.len() / 65_535 * 5 + 6);
    output.extend_from_slice(&[0x78, 0x01]);

    for (index, block) in input.chunks(65_535).enumerate() {
        let is_last = (index + 1) * 65_535 >= input.len();
        output.push(if is_last { 1 } else { 0 });
        let length = block.len() as u16;
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(block);
    }

    output.extend_from_slice(&adler32(input).to_be_bytes());
    output
}

fn decode_zlib_stored(encoded: &[u8]) -> Result<Vec<u8>, ImageDecodingError> {
    if encoded.get(..2) != Some(&[0x78, 0x01]) {
        return Err(ImageDecodingError::UnsupportedPng(
            "only ClipRiva stored-block zlib streams are supported",
        ));
    }

    let mut offset = 2;
    let mut output = Vec::new();
    loop {
        let header = *encoded
            .get(offset)
            .ok_or(ImageDecodingError::CorruptPng("truncated deflate header"))?;
        if header != 0 && header != 1 {
            return Err(ImageDecodingError::UnsupportedPng(
                "only aligned DEFLATE stored blocks are supported",
            ));
        }
        let is_last = header == 1;
        offset += 1;

        let length = u16::from_le_bytes(
            encoded
                .get(offset..offset + 2)
                .ok_or(ImageDecodingError::CorruptPng(
                    "truncated stored block length",
                ))?
                .try_into()
                .map_err(|_| ImageDecodingError::CorruptPng("invalid stored block length"))?,
        );
        let inverse = u16::from_le_bytes(
            encoded
                .get(offset + 2..offset + 4)
                .ok_or(ImageDecodingError::CorruptPng(
                    "truncated stored block checksum",
                ))?
                .try_into()
                .map_err(|_| ImageDecodingError::CorruptPng("invalid stored block checksum"))?,
        );
        if inverse != !length {
            return Err(ImageDecodingError::CorruptPng(
                "stored block checksum mismatch",
            ));
        }
        offset += 4;
        let data_end =
            offset
                .checked_add(usize::from(length))
                .ok_or(ImageDecodingError::CorruptPng(
                    "stored block length overflow",
                ))?;
        let data = encoded
            .get(offset..data_end)
            .ok_or(ImageDecodingError::CorruptPng(
                "truncated stored block data",
            ))?;
        output.extend_from_slice(data);
        offset = data_end;
        if is_last {
            break;
        }
    }

    let checksum = u32::from_be_bytes(
        encoded
            .get(offset..offset + 4)
            .ok_or(ImageDecodingError::CorruptPng("truncated Adler-32"))?
            .try_into()
            .map_err(|_| ImageDecodingError::CorruptPng("invalid Adler-32"))?,
    );
    if offset + 4 != encoded.len() {
        return Err(ImageDecodingError::CorruptPng("trailing zlib data"));
    }
    if adler32(&output) != checksum {
        return Err(ImageDecodingError::CorruptPng("Adler-32 mismatch"));
    }
    Ok(output)
}

fn crc32(kind: &[u8; 4], data: &[u8]) -> u32 {
    let mut value = 0xffff_ffff_u32;
    for byte in kind.iter().chain(data) {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = if value & 1 == 1 {
                (value >> 1) ^ 0xedb8_8320
            } else {
                value >> 1
            };
        }
    }
    !value
}

fn adler32(input: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in input {
        a = (a + u32::from(*byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::{decode_clipriva_rgba_png, encode_rgba_png, ImageEncodingError};

    #[test]
    fn encodes_a_valid_rgba_png_with_unfiltered_rows() {
        let png = encode_rgba_png(&[255, 0, 0, 255, 0, 255, 0, 128], 2, 1).unwrap();

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let chunks = parse_png_chunks(&png);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].0, *b"IHDR");
        assert_eq!(chunks[0].1, vec![0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
        assert_eq!(chunks[1].0, *b"IDAT");
        assert_eq!(chunks[2].0, *b"IEND");
        assert_eq!(
            decode_stored_zlib(&chunks[1].1),
            vec![0, 255, 0, 0, 255, 0, 255, 0, 128]
        );
        assert_eq!(
            decode_clipriva_rgba_png(&png).unwrap().pixels,
            vec![255, 0, 0, 255, 0, 255, 0, 128]
        );
    }

    #[test]
    fn rejects_incomplete_rgba_buffers() {
        assert_eq!(
            encode_rgba_png(&[0, 1, 2], 1, 1),
            Err(ImageEncodingError::InvalidRgbaLength {
                actual: 3,
                expected: 4,
            })
        );
        assert_eq!(
            encode_rgba_png(&[], 0, 1),
            Err(ImageEncodingError::ZeroDimensions)
        );
    }

    fn parse_png_chunks(png: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        let mut offset = 8;
        let mut chunks = Vec::new();
        while offset < png.len() {
            let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[offset + 4..offset + 8].try_into().unwrap();
            let data_start = offset + 8;
            let data_end = data_start + length;
            chunks.push((kind, png[data_start..data_end].to_vec()));
            offset = data_end + 4;
        }
        chunks
    }

    fn decode_stored_zlib(encoded: &[u8]) -> Vec<u8> {
        assert_eq!(&encoded[..2], &[0x78, 0x01]);
        let mut offset = 2;
        let mut output = Vec::new();
        loop {
            let final_block = encoded[offset] == 1;
            offset += 1;
            let length =
                u16::from_le_bytes(encoded[offset..offset + 2].try_into().unwrap()) as usize;
            let inverse = u16::from_le_bytes(encoded[offset + 2..offset + 4].try_into().unwrap());
            assert_eq!(inverse, !(length as u16));
            offset += 4;
            output.extend_from_slice(&encoded[offset..offset + length]);
            offset += length;
            if final_block {
                break;
            }
        }
        // The final four bytes are Adler-32. This parser only validates the
        // stored blocks; the encoder's PNG test above validates their content.
        assert_eq!(offset + 4, encoded.len());
        output
    }
}
