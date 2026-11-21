//! The 0.1 custody root, kept only to open the rows sealed before the custody
//! envelope: a 24-byte nonce, then the XChaCha20-Poly1305 ciphertext of the
//! three keys, with the DID as associated data. It opens and never seals.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::key_store::KeyBundle;

/// The length of a 0.1 nonce (192-bit).
const NONCE_LEN: usize = 24;

/// The 0.1 custody root: opens a 0.1 row, never seals one. Not `Clone`, wiped on
/// drop, and its `Debug` prints nothing of it.
///
/// ```compile_fail
/// fn assert_clone<T: Clone>() {}
/// assert_clone::<adapter_pg::LegacyRootKey>();
/// ```
///
/// ```
/// fn assert_named<T>() {}
/// assert_named::<adapter_pg::LegacyRootKey>();
/// ```
pub struct LegacyRootKey(Zeroizing<[u8; 32]>);

/// Why a 0.1 row didn't open. Names no DID and no byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum LegacyOpenError {
    /// Too short to hold a nonce, or its tag fails: wrong root, wrong DID or tampered.
    #[error("the 0.1 ciphertext doesn't open under the configured root")]
    Unopenable,
    /// It opens, but not to three 32-byte keys.
    #[error("the 0.1 plaintext is not three 32-byte keys")]
    NotThreeKeys,
}

impl LegacyRootKey {
    /// Takes the root bytes. Crate-private, not `From`: only the custody migrator builds one.
    pub(crate) fn new(root: Zeroizing<[u8; 32]>) -> Self {
        Self(root)
    }

    /// Opens `blob`, sealed for `did`, into the three keys in role order.
    pub(crate) fn open(&self, did: &str, blob: &[u8]) -> Result<KeyBundle, LegacyOpenError> {
        let (nonce_bytes, ciphertext) = blob
            .split_at_checked(NONCE_LEN)
            .ok_or(LegacyOpenError::Unopenable)?;
        let cipher = XChaCha20Poly1305::new((&*self.0).into());
        let payload = Payload {
            msg: ciphertext,
            aad: did.as_bytes(),
        };
        let plaintext = cipher
            .decrypt(XNonce::from_slice(nonce_bytes), payload)
            .map(Zeroizing::new)
            .map_err(|_| LegacyOpenError::Unopenable)?;
        KeyBundle::try_from(plaintext.as_slice()).map_err(|_| LegacyOpenError::NotThreeKeys)
    }
}

impl std::fmt::Debug for LegacyRootKey {
    /// Prints nothing of the root.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LegacyRootKey(<redacted>)")
    }
}

#[cfg(test)]
mod tests;
