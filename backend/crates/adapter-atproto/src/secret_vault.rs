//! Envelope encryption of this crate's at-rest OAuth secrets.
//!
//! Every blob [`AtprotoAuthStore`](crate::AtprotoAuthStore) writes is sealed
//! with XChaCha20-Poly1305 under a 32-byte root key held outside the database,
//! so a database read alone yields no usable secret. The root key is the same
//! one adapter-pg's `key_vault` uses; it comes from config/env (DEV-ONLY).

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

/// XChaCha20-Poly1305 nonce length (192-bit) — wide enough for random nonces
/// with no counter or state.
const NONCE_LEN: usize = 24;

/// The 32-byte root key that seals every at-rest OAuth secret. Held in memory
/// only; [`Debug`] is redacted so it can never reach a log line.
#[derive(Clone)]
pub struct SecretVault([u8; 32]);

impl std::fmt::Debug for SecretVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretVault(<redacted>)")
    }
}

impl SecretVault {
    /// Build the vault from exactly 32 bytes; any other length errors, so a
    /// misconfigured secret fails at boot rather than weakening encryption.
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let arr: [u8; 32] = bytes.try_into().map_err(|_| {
            anyhow::anyhow!(
                "oauth store root key must be exactly 32 bytes, got {}",
                bytes.len()
            )
        })?;
        Ok(Self(arr))
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new((&self.0).into())
    }

    /// Seal `plaintext` into an opaque blob: a fresh random nonce followed by
    /// the AEAD ciphertext. `aad` is bound in as associated data — not stored,
    /// but required identically on [`open`](SecretVault::open) — so passing the
    /// row's key ties the blob to its row and blocks cross-row swaps.
    pub fn seal(&self, aad: &[u8], plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher()
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to seal oauth secret"))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(nonce.as_slice());
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    /// Open a blob produced by [`seal`](SecretVault::seal) under the identical
    /// `aad`; the plaintext is zeroized on drop. Errors — never returning
    /// plaintext — on a malformed blob, a mismatched `aad`, or a failed tag.
    pub fn open(&self, aad: &[u8], blob: &[u8]) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        if blob.len() < NONCE_LEN {
            anyhow::bail!("sealed oauth blob too short");
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce = XNonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher()
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| {
                anyhow::anyhow!(
                    "failed to open oauth secret (bad root key, wrong row, or tampered)"
                )
            })?;
        Ok(Zeroizing::new(plaintext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AAD: &[u8] = b"atproto_oauth.client_session\0did:plc:alice";
    const PLAINTEXT: &[u8] = b"{\"refresh_token\":\"refresh-token\"}";

    // Round-trip: seal then open under the same AAD yields the same plaintext.
    #[test]
    fn seal_open_round_trips() {
        let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
        let blob = vault.seal(AAD, PLAINTEXT).unwrap();
        assert_eq!(vault.open(AAD, &blob).unwrap().as_slice(), PLAINTEXT);
    }

    // The sealed blob must NOT contain the plaintext bytes — this is the whole
    // point of encryption at rest.
    #[test]
    fn sealed_blob_is_not_plaintext() {
        let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
        let blob = vault.seal(AAD, PLAINTEXT).unwrap();
        assert_ne!(blob.as_slice(), PLAINTEXT);
        // The distinctive secret run does not survive into the ciphertext.
        let needle = b"refresh-token";
        assert!(
            !blob.windows(needle.len()).any(|w| w == needle),
            "plaintext secret leaked into the sealed blob"
        );
    }

    // A different root key cannot open the blob (AEAD tag fails).
    #[test]
    fn wrong_root_key_cannot_open() {
        let blob = SecretVault::from_bytes(&[7u8; 32])
            .unwrap()
            .seal(AAD, PLAINTEXT)
            .unwrap();
        assert!(
            SecretVault::from_bytes(&[8u8; 32])
                .unwrap()
                .open(AAD, &blob)
                .is_err()
        );
    }

    // The AAD is bound: a blob sealed under one row key cannot be opened under
    // another, so sealed secrets cannot be swapped across rows.
    #[test]
    fn blob_cannot_be_opened_under_a_different_aad() {
        let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
        let blob = vault.seal(AAD, PLAINTEXT).unwrap();
        assert!(
            vault
                .open(b"atproto_oauth.client_session\0did:plc:mallory", &blob)
                .is_err()
        );
    }

    // A tampered blob fails to open (authenticity).
    #[test]
    fn tampered_blob_fails() {
        let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
        let mut blob = vault.seal(AAD, PLAINTEXT).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;
        assert!(vault.open(AAD, &blob).is_err());
    }

    // A blob shorter than the nonce is rejected, not indexed out of bounds.
    #[test]
    fn short_blob_fails_closed() {
        let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
        assert!(vault.open(AAD, &[0u8; NONCE_LEN - 1]).is_err());
    }

    // A wrong-length root key is rejected at construction.
    #[test]
    fn root_key_must_be_32_bytes() {
        assert!(SecretVault::from_bytes(&[0u8; 16]).is_err());
        assert!(SecretVault::from_bytes(&[0u8; 32]).is_ok());
    }

    // The vault's Debug must never reveal its bytes.
    #[test]
    fn vault_debug_is_redacted() {
        let vault = SecretVault::from_bytes(&[0xCD; 32]).unwrap();
        let shown = format!("{vault:?}");
        assert_eq!(shown, "SecretVault(<redacted>)");
        assert!(!shown.contains("cd") && !shown.contains("205"));
    }
}
