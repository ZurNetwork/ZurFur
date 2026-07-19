//! Envelope encryption of this store's at-rest OAuth secrets under a **root key**.
//!
//! The `atproto_oauth` rows hold live upstream credentials on the user's behalf:
//! an established session carries the DPoP **private signing key** plus the
//! long-lived **refresh token** and access token; an in-flight request carries
//! the PKCE verifier + DPoP key. A read of those rows in the clear (leaked
//! backup, read replica, injection read gadget) is a *renewable* PDS-session
//! takeover — it bypasses the encrypted `did:plc` custody store entirely. So,
//! exactly as the account custody keys are sealed before they touch disk, every
//! `data` blob [`AtprotoAuthStore`](crate::AtprotoAuthStore) writes is sealed
//! with an AEAD (XChaCha20-Poly1305) under a 32-byte **root key** held *outside*
//! the database — a database compromise alone yields no usable secret.
//!
//! # A deliberate mirror of adapter-pg's `key_vault`, not a shared type
//!
//! This is a faithful mirror of adapter-pg's `key_vault::RootKey` (the custody
//! encryptor, DD/26804226): same AEAD, same 24-byte random-nonce-prefixed blob
//! format, same associated-data binding, and — critically — the **same single
//! root-key source** (`ZURFUR_DID_KEY_ROOT_KEY`, injected by `api`). It is not
//! *the same type* because the ports-and-adapters rule forbids one adapter
//! depending on another (`adapter-pg` is a dev-only dependency here), and this
//! security fix is deliberately confined to `adapter-atproto`. Hoisting the
//! envelope primitive into `domain` so both boundaries share one implementation
//! is the recommended follow-up; it is out of scope for this PR.
//!
//! # Root key custody — DEV-ONLY today, KMS next
//!
//! Same key, same caveat as custody: a config/env root key is acceptable **only**
//! pre-alpha; hardening it into a cloud KMS/HSM is the URGENT follow-up ZMVP-53.
//! Because this store reuses the one custody root key, `api`'s
//! `ensure_custody_hardened` boot guard (which refuses to run real-identity
//! configurations under dev-only key custody) already covers this store too.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

/// XChaCha20-Poly1305 nonce length (192-bit) — large enough that random nonces
/// never collide in practice, so no counter/state is needed even though this key
/// also seals custody records (the combined message set stays far under the
/// birthday bound).
const NONCE_LEN: usize = 24;

/// The 32-byte root key that seals every at-rest OAuth secret. Held in memory
/// only; sourced from config/env in v1 (DEV-ONLY — see module docs, ZMVP-53).
/// [`Debug`] is redacted so the root key can never reach a log line.
#[derive(Clone)]
pub struct SecretVault([u8; 32]);

impl std::fmt::Debug for SecretVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretVault(<redacted>)")
    }
}

impl SecretVault {
    /// Build the vault from exactly 32 bytes (the same decoded
    /// `ZURFUR_DID_KEY_ROOT_KEY` the custody store uses). Errors on any other
    /// length so a misconfigured secret fails loudly at boot rather than silently
    /// weakening encryption.
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

    /// Seal `plaintext` into an opaque blob: a fresh random nonce followed by the
    /// AEAD ciphertext. Only a holder of this root key can [`open`](SecretVault::open)
    /// it.
    ///
    /// `aad` is bound in as AEAD **associated data** — it is *not* stored in the
    /// blob, but the tag check on `open` fails unless the identical `aad` is
    /// supplied. Passing each row's key as `aad` cryptographically ties a blob to
    /// its row: an attacker with database write access cannot move one row's
    /// sealed secret onto another row (the tag fails under the moved-to key).
    /// Defense-in-depth on top of the core property (a DB read alone yields no
    /// usable secret).
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

    /// Open a blob produced by [`seal`](SecretVault::seal). `aad` must be identical
    /// to what the blob was sealed under (the row key). The plaintext is returned
    /// in a [`Zeroizing`] buffer so the decrypted secret is wiped from the heap on
    /// drop.
    ///
    /// Errors — and **never** returns plaintext — if the blob is malformed, the
    /// `aad` does not match, or the AEAD tag fails (wrong root key or tampering).
    /// This is the fail-closed contract: a legacy/plaintext value in the column is
    /// not valid ciphertext, so it errors here rather than being silently passed
    /// through as if it had been decrypted.
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
