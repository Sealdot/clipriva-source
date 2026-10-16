//! Stable lowercase encoding for persisted SHA-256 content addresses.

use sha2::{Digest, Sha256};

pub(crate) fn sha256_hex(input: impl AsRef<[u8]>) -> String {
    lower_hex(Sha256::digest(input))
}

pub(crate) fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_sha256_addresses_keep_the_existing_encoding() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut digest = Sha256::new();
        digest.update(b"a");
        digest.update(b"bc");
        assert_eq!(lower_hex(digest.finalize()), sha256_hex(b"abc"));
    }

    #[test]
    fn hexadecimal_encoding_preserves_zero_padding_and_high_bytes() {
        assert_eq!(lower_hex([0, 1, 15, 16, 128, 255]), "00010f1080ff");
    }
}
