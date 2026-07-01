//! The private keys Zurfur custodies for a minted account `did:plc`.
//!
//! When Zurfur mints an account's sovereign identity (see [`crate::ports::DidMinter`])
//! it generates a small set of **secp256k1** keypairs and keeps the private
//! halves so it can operate the DID on the account's behalf — signing the genesis
//! operation now and, later, rotations and `alsoKnownAs` updates. Per the custody
//! model (DD *did:plc Identity Custody, Minting & Credible Exit*, DESIGN/26804226)
//! these keys are **per-account, never a shared platform key**, and are held
//! **envelope-encrypted at rest** behind the [`crate::ports::KeyStore`] port.
//!
//! This module is the domain-side shape of that key set. It carries no crypto: it
//! is the plaintext material in transit between the minter (which generates it)
//! and the [`KeyStore`](crate::ports::KeyStore) (which encrypts it before it
//! touches disk). Secrets [`Zeroize`] on drop so a decrypted key does not linger
//! in freed memory.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// One secp256k1 private key, held as its raw 32-byte big-endian scalar (the form
/// `atrium_crypto`'s keypair `export`/`import` round-trips). Zeroized on drop; its
/// [`Debug`] is redacted so key material can never reach a log line.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey(Vec<u8>);

impl SecretKey {
    /// Wrap raw private-key bytes (the 32-byte secp256k1 scalar). No validation:
    /// the bytes come from a trusted place — a freshly generated keypair's
    /// `export`, or a decrypted [`KeyStore`](crate::ports::KeyStore) record.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The raw private-key bytes, to hand to the crypto adapter for signing or to
    /// the [`KeyStore`](crate::ports::KeyStore) for encryption. Treat as secret:
    /// never log, never persist unencrypted.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

/// Redacted on purpose: printing key material — even in a debug log or a panic
/// message — would defeat the custody model. Shows only that a key is present.
impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

/// The full set of secp256k1 private keys Zurfur holds for one minted `did:plc`,
/// named by the role each plays in the genesis operation. Ordering of the two
/// rotation keys is **load-bearing** — a DID's rotation keys are listed in
/// *descending authority*, and recovery works by a higher-authority key
/// overriding a lower one within the PLC 72-hour window (DD/26804226).
///
/// - `cold_recovery` — `rotationKeys[0]`, highest authority. Reserved for
///   recovery; kept coldest (never used on the routine signing path). Index 0 is
///   deliberately *below* a future user-held recovery key, which enrolls above it
///   for credible exit (ZMVP-52).
/// - `operational` — `rotationKeys[1]`. Zurfur's day-to-day key: signs the genesis
///   operation and future updates.
/// - `signing` — the `#atproto` verification method. Included for forward-compat
///   (a repo/PDS attached later signs records with it); unused in identity-only v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKeys {
    /// `rotationKeys[0]` — highest-authority recovery key (DD/26804226 B2).
    pub cold_recovery: SecretKey,
    /// `rotationKeys[1]` — Zurfur's operational key; signs operations.
    pub operational: SecretKey,
    /// The `#atproto` signing key (verification method); forward-compat (B3).
    pub signing: SecretKey,
}
