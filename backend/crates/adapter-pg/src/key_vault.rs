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
mod tests;
