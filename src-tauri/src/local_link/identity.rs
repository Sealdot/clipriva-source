//! Long-term Local Link identity.
//!
//! The private Noise static key is stored only in the macOS data-protection
//! Keychain. SQLite receives the public fingerprint of trusted peers, never a
//! local private key or a session key.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::error::{AppError, AppResult};

use super::NOISE_PARAMS;

const IDENTITY_BLOB_VERSION: u8 = 1;
const KEY_BYTES: usize = 32;
const IDENTITY_BLOB_BYTES: usize = 1 + KEY_BYTES + KEY_BYTES;

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "com.clipriva.desktop.local-link.identity";
#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "noise-static-v1";

/// Intentionally has no `Debug` implementation. Dropping the value zeroes the
/// process copy of the private key.
pub(crate) struct LocalIdentity {
    private_key: Zeroizing<Vec<u8>>,
    public_key: Vec<u8>,
    fingerprint: String,
}

impl LocalIdentity {
    pub(crate) fn load_or_create(allow_create: bool) -> AppResult<Self> {
        let store = PlatformIdentityStore;
        Self::load_or_create_with_store(&store, allow_create)
    }

    fn load_or_create_with_store(
        store: &impl IdentityStore,
        allow_create: bool,
    ) -> AppResult<Self> {
        match store.load()? {
            Some(blob) => Self::from_blob(blob),
            None if !allow_create => Err(AppError::InvalidInput(
                "The Local Link identity is missing while trusted devices still exist. Reset Local Link identity and pair those devices again."
                    .to_owned(),
            )),
            None => {
                let identity = Self::generate()?;
                store.save(&identity.to_blob())?;
                Ok(identity)
            }
        }
    }

    pub(crate) fn delete_stored() -> AppResult<()> {
        PlatformIdentityStore.delete()
    }

    #[cfg(test)]
    pub(crate) fn generate_for_test() -> AppResult<Self> {
        Self::generate()
    }

    fn generate() -> AppResult<Self> {
        let params = NOISE_PARAMS.parse().map_err(|_| {
            AppError::InvalidInput("Local Link Noise parameters are invalid.".to_owned())
        })?;
        let builder = snow::Builder::new(params);
        let keypair = builder.generate_keypair().map_err(|_| {
            AppError::InvalidInput("Could not generate the Local Link identity key.".to_owned())
        })?;
        if keypair.private.len() != KEY_BYTES || keypair.public.len() != KEY_BYTES {
            return Err(AppError::InvalidInput(
                "The Local Link identity key has an unexpected size.".to_owned(),
            ));
        }
        verify_public_key(&keypair.private, &keypair.public)?;
        Ok(Self::new(keypair.private, keypair.public))
    }

    fn from_blob(mut blob: Vec<u8>) -> AppResult<Self> {
        if blob.len() != IDENTITY_BLOB_BYTES || blob[0] != IDENTITY_BLOB_VERSION {
            blob.zeroize();
            return Err(AppError::InvalidInput(
                "The Local Link Keychain identity is invalid; Local Link remains off.".to_owned(),
            ));
        }
        let private_key = blob[1..1 + KEY_BYTES].to_vec();
        let public_key = blob[1 + KEY_BYTES..].to_vec();
        blob.zeroize();
        verify_public_key(&private_key, &public_key)?;
        Ok(Self::new(private_key, public_key))
    }

    fn new(private_key: Vec<u8>, public_key: Vec<u8>) -> Self {
        let fingerprint = fingerprint(&public_key);
        Self {
            private_key: Zeroizing::new(private_key),
            public_key,
            fingerprint,
        }
    }

    fn to_blob(&self) -> Zeroizing<Vec<u8>> {
        let mut blob = Zeroizing::new(Vec::with_capacity(IDENTITY_BLOB_BYTES));
        blob.push(IDENTITY_BLOB_VERSION);
        blob.extend_from_slice(self.private_key());
        blob.extend_from_slice(self.public_key());
        blob
    }

    pub(crate) fn private_key(&self) -> &[u8] {
        self.private_key.as_slice()
    }

    pub(crate) fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    pub(crate) fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

pub(crate) fn fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    // Twelve bytes (96 bits) is compact enough for the UI while remaining far
    // stronger than a human-comparison code. Grouping is display-only.
    digest[..12]
        .chunks(2)
        .map(|chunk| format!("{:02X}{:02X}", chunk[0], chunk[1]))
        .collect::<Vec<_>>()
        .join("-")
}

fn verify_public_key(private_key: &[u8], stored_public_key: &[u8]) -> AppResult<()> {
    let private_bytes: [u8; KEY_BYTES] = private_key.try_into().map_err(|_| {
        AppError::InvalidInput(
            "The Local Link Keychain identity is invalid; Local Link remains off.".to_owned(),
        )
    })?;
    let derived = PublicKey::from(&StaticSecret::from(private_bytes));
    if derived.as_bytes().ct_eq(stored_public_key).unwrap_u8() != 1 {
        return Err(AppError::InvalidInput(
            "The Local Link Keychain identity failed validation; Local Link remains off."
                .to_owned(),
        ));
    }
    Ok(())
}

trait IdentityStore {
    fn load(&self) -> AppResult<Option<Vec<u8>>>;
    fn save(&self, blob: &[u8]) -> AppResult<()>;
    fn delete(&self) -> AppResult<()>;
}

struct PlatformIdentityStore;

#[cfg(target_os = "macos")]
impl IdentityStore for PlatformIdentityStore {
    fn load(&self) -> AppResult<Option<Vec<u8>>> {
        use security_framework::passwords::{generic_password, PasswordOptions};

        let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
        options.set_access_synchronized(Some(false));
        match generic_password(options) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.code() == -25300 => Ok(None), // errSecItemNotFound
            Err(_) => Err(AppError::InvalidInput(
                "Could not read the Local Link identity from macOS Keychain; Local Link remains off."
                    .to_owned(),
            )),
        }
    }

    fn save(&self, blob: &[u8]) -> AppResult<()> {
        use security_framework::access_control::{ProtectionMode, SecAccessControl};
        use security_framework::passwords::{set_generic_password_options, PasswordOptions};

        let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
        options.set_access_synchronized(Some(false));
        let access_control = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            0,
        )
        .map_err(|_| {
            AppError::InvalidInput(
                "Could not apply the Local Link Keychain protection boundary; Local Link remains off."
                    .to_owned(),
            )
        })?;
        options.set_access_control(access_control);
        options.set_label("ClipRiva Local Link identity");
        options.set_description("Long-term on-device Noise identity for trusted Local Link peers");
        set_generic_password_options(blob, options).map_err(|_| {
            AppError::InvalidInput(
                "Could not save the Local Link identity in macOS Keychain; Local Link remains off."
                    .to_owned(),
            )
        })
    }

    fn delete(&self) -> AppResult<()> {
        use security_framework::passwords::{delete_generic_password_options, PasswordOptions};

        let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
        options.set_access_synchronized(Some(false));
        match delete_generic_password_options(options) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25300 => Ok(()), // errSecItemNotFound
            Err(_) => Err(AppError::InvalidInput(
                "Could not delete the Local Link identity from macOS Keychain; Local Link remains off."
                    .to_owned(),
            )),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl IdentityStore for PlatformIdentityStore {
    fn load(&self) -> AppResult<Option<Vec<u8>>> {
        Err(AppError::InvalidInput(
            "Local Link identity is available on macOS only.".to_owned(),
        ))
    }

    fn save(&self, _blob: &[u8]) -> AppResult<()> {
        Err(AppError::InvalidInput(
            "Local Link identity is available on macOS only.".to_owned(),
        ))
    }

    fn delete(&self) -> AppResult<()> {
        Err(AppError::InvalidInput(
            "Local Link identity is available on macOS only.".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Default)]
    struct MemoryIdentityStore {
        blob: RefCell<Option<Vec<u8>>>,
        save_count: Cell<usize>,
    }

    impl IdentityStore for MemoryIdentityStore {
        fn load(&self) -> AppResult<Option<Vec<u8>>> {
            Ok(self.blob.borrow().clone())
        }

        fn save(&self, blob: &[u8]) -> AppResult<()> {
            self.save_count.set(self.save_count.get() + 1);
            *self.blob.borrow_mut() = Some(blob.to_vec());
            Ok(())
        }

        fn delete(&self) -> AppResult<()> {
            self.blob.borrow_mut().take();
            Ok(())
        }
    }

    #[test]
    fn identity_blob_round_trip_is_stable_and_fingerprinted() {
        let identity = LocalIdentity::generate_for_test().unwrap();
        let blob = identity.to_blob();
        let restored = LocalIdentity::from_blob(blob.to_vec()).unwrap();
        assert_eq!(restored.private_key(), identity.private_key());
        assert_eq!(restored.public_key(), identity.public_key());
        assert_eq!(restored.fingerprint(), identity.fingerprint());
        assert_eq!(restored.fingerprint().len(), 29);
    }

    #[test]
    fn malformed_keychain_blob_fails_closed() {
        match LocalIdentity::from_blob(vec![1, 2, 3]) {
            Ok(_) => panic!("malformed identity must fail closed"),
            Err(error) => assert!(error.to_string().contains("remains off")),
        }
    }

    #[test]
    fn public_key_mismatch_in_keychain_blob_fails_closed() {
        let identity = LocalIdentity::generate_for_test().unwrap();
        let mut blob = identity.to_blob().to_vec();
        blob[IDENTITY_BLOB_BYTES - 1] ^= 0x80;
        match LocalIdentity::from_blob(blob) {
            Ok(_) => panic!("mismatched public key must fail closed"),
            Err(error) => assert!(error.to_string().contains("failed validation")),
        }
    }

    #[test]
    fn missing_identity_with_existing_trust_does_not_silently_generate() {
        let store = MemoryIdentityStore::default();
        match LocalIdentity::load_or_create_with_store(&store, false) {
            Ok(_) => panic!("missing identity with trusted peers must fail closed"),
            Err(error) => assert!(error.to_string().contains("trusted devices still exist")),
        }
        assert_eq!(store.save_count.get(), 0);
        assert!(store.blob.borrow().is_none());
    }

    #[test]
    fn first_use_without_trust_generates_and_persists_once() {
        let store = MemoryIdentityStore::default();
        let identity = LocalIdentity::load_or_create_with_store(&store, true).unwrap();
        assert_eq!(store.save_count.get(), 1);
        let restored = LocalIdentity::load_or_create_with_store(&store, false).unwrap();
        assert_eq!(identity.public_key(), restored.public_key());
        assert_eq!(store.save_count.get(), 1);
    }
}
