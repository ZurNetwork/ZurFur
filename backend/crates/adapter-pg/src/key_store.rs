//! [`PgKeyStore`] — PostgreSQL custody store for minted `did:plc` keys, encrypted
//! at rest.
//!
//! Implements the [`KeyStore`] port over the `account_keys` table. Every account's
//! secp256k1 custody keys are **envelope-encrypted** under a [`RootKey`] (see
//! [`crate::key_vault`]) before they are written, so the database never holds
//! plaintext key material. The write is a single-row, pool-backed insert performed
//! *during minting*, **before the account row exists** — the account's DID is
//! derived from these very keys, so there is no account transaction yet to join
//! (same-store *temporal ordering*, not a cross-store concern). It is therefore
//! deliberately outside the account [`UnitOfWork`](domain::ports::UnitOfWork), like
//! the profile-cache fill (DD `24150017`). Minting stores keys, then — as a
//! distinct step — submits the operation to a public directory (that latter pair
//! *is* the cross-boundary dual write, run as separate retryable steps).
//!
//! The SQL lives in `queries/key_store/`; the typed functions are generated
//! against the migrated schema (see [`crate::queries`]).

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{account_keys::AccountKeys, did::Did},
    ports::KeyStore,
};
use sqlx::PgPool;

use crate::key_vault::RootKey;
use crate::queries::key_store as sql;

/// PostgreSQL [`KeyStore`]: wraps custody keys under a [`RootKey`] and persists the
/// sealed blob in `account_keys`. Holds the pool and the root key; both are cheap
/// to clone. Injected by `api` from config (the root key is DEV-ONLY in v1 — a
/// cloud-KMS-backed [`KeyStore`] replaces this before real accounts, ZMVP-53).
pub struct PgKeyStore {
    pool: PgPool,
    root: RootKey,
}

impl PgKeyStore {
    /// Build the store over a connection `pool` and the `root` key that encrypts
    /// every custody record.
    pub fn new(pool: PgPool, root: RootKey) -> Self {
        Self { pool, root }
    }
}

#[async_trait]
impl KeyStore for PgKeyStore {
    /// Envelope-encrypt `keys` under the root key and insert them for `did`.
    /// `key_version` records the wrapping scheme so keys can be re-wrapped under a
    /// new root key (or KMS) later. One DID mints once, so a duplicate insert is a
    /// constraint error (the PK), surfaced to the caller.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()> {
        let wrapped = self.root.wrap(did.as_str(), keys)?;
        sql::put(&self.pool, did.as_str(), &wrapped, 1i32, Utc::now()).await?;
        Ok(())
    }

    /// Load the sealed blob for `did` and open it back into [`AccountKeys`], or
    /// `None` if unknown. Decryption failure (wrong root key or tampering) is an
    /// error, not a `None`.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>> {
        let wrapped = sql::get(&self.pool, did.as_str()).await?;

        match wrapped {
            Some(wrapped) => Ok(Some(self.root.unwrap(did.as_str(), &wrapped)?)),
            None => Ok(None),
        }
    }
}
