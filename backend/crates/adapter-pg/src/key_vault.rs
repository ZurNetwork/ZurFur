//! Envelope encryption of custody key material under a **root key**
//! (XChaCha20-Poly1305), before [`AccountKeys`] touch disk via
//! [`PgKeyStore`](crate::key_store::PgKeyStore). The root key is
//! DEV-ONLY (config/env) in v1 — a KMS/HSM must back it before any real
//! account is minted.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use domain::elements::account_keys::{AccountKeys, SecretKey};
use zeroize::Zeroizing;

/// A secp256k1 private scalar is 32 bytes; the bundle concatenates the three
/// keys in role order.
const SECRET_LEN: usize = 32;
/// XChaCha20-Poly1305 nonce length (192-bit); random nonces don't need a counter.
const NONCE_LEN: usize = 24;

/// The 32-byte root key that wraps every account's custody keys. In-memory
/// only. [`Debug`] is redacted so it can never reach a log line.
#[derive(Clone)]
pub struct RootKey([u8; 32]);

impl std::fmt::Debug for RootKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RootKey(<redacted>)")
    }
}

impl RootKey {
    /// Build a root key from exactly 32 bytes. Errors on any other length so
    /// a misconfigured secret fails loudly at boot.
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let arr: [u8; 32] = bytes.try_into().map_err(|_| {
            anyhow::anyhow!("root key must be exactly 32 bytes, got {}", bytes.len())
        })?;
        Ok(Self(arr))
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new((&self.0).into())
    }

    /// Seals an account's custody keys into an opaque blob: a random nonce
    /// followed by the AEAD ciphertext of `[cold ‖ operational ‖ signing]`.
    /// The `did` is bound as AEAD associated data, so a blob cannot be moved
    /// onto another account's row — `unwrap` fails under the wrong DID.
    pub fn wrap(&self, did: &str, keys: &AccountKeys) -> anyhow::Result<Vec<u8>> {
        // `Zeroizing` wipes the plaintext key bundle on drop.
        let mut plaintext = Zeroizing::new(Vec::with_capacity(3 * SECRET_LEN));
        for secret in [&keys.cold_recovery, &keys.operational, &keys.signing] {
            let bytes = secret.expose();
            if bytes.len() != SECRET_LEN {
                anyhow::bail!(
                    "expected {SECRET_LEN}-byte secp256k1 key, got {}",
                    bytes.len()
                );
            }
            plaintext.extend_from_slice(bytes);
        }

        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher()
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.as_slice(),
                    aad: did.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to seal custody keys"))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(nonce.as_slice());
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    /// Opens a blob from [`wrap`](RootKey::wrap) back into [`AccountKeys`].
    /// `did` must match the sealing DID. Errors on a malformed blob, wrong
    /// DID, or failed AEAD tag (wrong root key or tampering).
    pub fn unwrap(&self, did: &str, blob: &[u8]) -> anyhow::Result<AccountKeys> {
        if blob.len() < NONCE_LEN {
            anyhow::bail!("wrapped key blob too short");
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce = XNonce::from_slice(nonce_bytes);
        // `Zeroizing` wipes the decrypted key bundle on drop, after the per-role
        // `SecretKey`s are copied out of it below.
        let plaintext = Zeroizing::new(
            self.cipher()
                .decrypt(
                    nonce,
                    Payload {
                        msg: ciphertext,
                        aad: did.as_bytes(),
                    },
                )
                .map_err(|_| {
                    anyhow::anyhow!(
                        "failed to open custody keys (bad root key, wrong DID, or tampered)"
                    )
                })?,
        );

        if plaintext.len() != 3 * SECRET_LEN {
            anyhow::bail!(
                "decrypted custody bundle has unexpected length {}",
                plaintext.len()
            );
        }
        Ok(AccountKeys {
            cold_recovery: SecretKey::new(plaintext[0..SECRET_LEN].to_vec()),
            operational: SecretKey::new(plaintext[SECRET_LEN..2 * SECRET_LEN].to_vec()),
            signing: SecretKey::new(plaintext[2 * SECRET_LEN..3 * SECRET_LEN].to_vec()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> AccountKeys {
        AccountKeys {
            cold_recovery: SecretKey::new(vec![0xAA; 32]),
            operational: SecretKey::new(vec![0xBB; 32]),
            signing: SecretKey::new(vec![0xCC; 32]),
        }
    }

    const DID: &str = "did:plc:alice";

    // Round-trip: wrap then unwrap under the same DID yields the same keys.
    #[test]
    fn wrap_unwrap_round_trips() {
        let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
        let blob = root.wrap(DID, &keys()).unwrap();
        assert_eq!(root.unwrap(DID, &blob).unwrap(), keys());
    }

    // The sealed blob must NOT contain the plaintext key bytes — this is the whole
    // point of encryption at rest.
    #[test]
    fn wrapped_blob_is_not_plaintext() {
        let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
        let blob = root.wrap(DID, &keys()).unwrap();
        // None of the three 32-byte plaintext runs appears in the ciphertext.
        for byte in [0xAAu8, 0xBB, 0xCC] {
            let run = vec![byte; 32];
            assert!(
                !blob.windows(32).any(|w| w == run.as_slice()),
                "plaintext key bytes ({byte:#x}) leaked into the wrapped blob"
            );
        }
    }

    // A different root key cannot open the blob (AEAD tag fails).
    #[test]
    fn wrong_root_key_cannot_unwrap() {
        let blob = RootKey::from_bytes(&[7u8; 32])
            .unwrap()
            .wrap(DID, &keys())
            .unwrap();
        assert!(
            RootKey::from_bytes(&[8u8; 32])
                .unwrap()
                .unwrap(DID, &blob)
                .is_err()
        );
    }

    // The DID is bound as AEAD associated data: a blob sealed for one DID cannot be
    // opened under another, so custody rows cannot be swapped across accounts.
    #[test]
    fn blob_cannot_be_opened_under_a_different_did() {
        let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
        let blob = root.wrap(DID, &keys()).unwrap();
        assert!(root.unwrap("did:plc:mallory", &blob).is_err());
    }

    // A tampered blob fails to open (authenticity).
    #[test]
    fn tampered_blob_fails() {
        let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
        let mut blob = root.wrap(DID, &keys()).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;
        assert!(root.unwrap(DID, &blob).is_err());
    }

    // A wrong-length root key is rejected at construction.
    #[test]
    fn root_key_must_be_32_bytes() {
        assert!(RootKey::from_bytes(&[0u8; 16]).is_err());
        assert!(RootKey::from_bytes(&[0u8; 32]).is_ok());
    }

    // The root key's Debug must never reveal its bytes.
    #[test]
    fn root_key_debug_is_redacted() {
        let root = RootKey::from_bytes(&[0xCD; 32]).unwrap();
        let shown = format!("{root:?}");
        assert_eq!(shown, "RootKey(<redacted>)");
        assert!(!shown.contains("cd") && !shown.contains("205"));
    }
}
