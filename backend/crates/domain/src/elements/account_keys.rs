//! The private secp256k1 keys Zurfur custodies for a minted account `did:plc`,
//! per-account and never a shared platform key. (DD 26804226)
//!
//! This module carries no crypto: it is the plaintext material in transit
//! between the minter that generates it and the
//! [`KeyStore`](crate::ports::KeyStore) that envelope-encrypts it before it
//! touches disk. Secrets [`Zeroize`] on drop.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// One secp256k1 private key, held as its raw 32-byte big-endian scalar.
/// Zeroized on drop, and its [`Debug`] is redacted so key material can never
/// reach a log line.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey(Vec<u8>);

impl SecretKey {
    /// Wrap raw private-key bytes. No validation — the bytes come from a
    /// trusted place.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The raw private-key bytes, for the crypto adapter or the
    /// [`KeyStore`](crate::ports::KeyStore). Never log, never persist
    /// unencrypted.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

/// Redacted on purpose: shows only that a key is present.
impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

/// The full set of secp256k1 private keys Zurfur holds for one minted
/// `did:plc`, named by the role each plays in the genesis operation. The
/// rotation-key order is **load-bearing**: rotation keys are listed in
/// descending authority, and recovery works by a higher-authority key
/// overriding a lower one within the PLC window. (DD 26804226)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKeys {
    /// `rotationKeys[0]` — the highest-authority recovery key, kept coldest.
    pub cold_recovery: SecretKey,
    /// `rotationKeys[1]` — Zurfur's operational key; signs operations.
    pub operational: SecretKey,
    /// The `#atproto` signing key; forward-compat, unused in v1.
    pub signing: SecretKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    // A SecretKey's Debug must never reveal its bytes.
    #[test]
    fn secret_key_debug_is_redacted() {
        let key = SecretKey::new(vec![0xAB; 32]);
        let shown = format!("{key:?}");
        assert_eq!(shown, "SecretKey(<redacted>)");
        assert!(!shown.contains("ab"));
        assert!(!shown.contains("171"));
    }

    // Every AccountKeys field is a redacted SecretKey, so the bundle is safe.
    #[test]
    fn account_keys_debug_redacts_every_field() {
        let keys = AccountKeys {
            cold_recovery: SecretKey::new(vec![1; 32]),
            operational: SecretKey::new(vec![2; 32]),
            signing: SecretKey::new(vec![3; 32]),
        };
        let shown = format!("{keys:?}");
        assert_eq!(shown.matches("<redacted>").count(), 3);
        assert!(!shown.contains("[1, 1, 1"));
    }
}
