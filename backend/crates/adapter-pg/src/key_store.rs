//! [`PgKeyStore`] — PostgreSQL custody store for minted `did:plc` keys,
//! implementing [`KeyStore`] over `account_keys`. Keys are envelope-encrypted
//! under a [`RootKey`] (see [`crate::key_vault`]) before they're written. The
//! write is pool-backed and runs during minting, *before* the account row
//! exists, so it's deliberately outside the account
//! [`UnitOfWork`](domain::ports::UnitOfWork).

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{account_keys::AccountKeys, did::Did},
    ports::KeyStore,
};
use sqlx::PgPool;

use crate::key_vault::RootKey;
use crate::queries::key_store as sql;

/// PostgreSQL [`KeyStore`]: wraps custody keys under a [`RootKey`] and
/// persists the sealed blob in `account_keys`. The root key is DEV-ONLY in
/// v1 — a cloud-KMS-backed [`KeyStore`] replaces this before real accounts.
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
    /// Envelope-encrypts `keys` under the root key and inserts them for `did`.
    /// One DID mints once, so a duplicate insert is a constraint error.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()> {
        let wrapped = self.root.wrap(did.as_str(), keys)?;
        sql::put(&self.pool, did.as_str(), &wrapped, 1i32, Utc::now()).await?;
        Ok(())
    }

    /// Loads the sealed blob for `did` and opens it into [`AccountKeys`], or
    /// `None` if unknown. Decryption failure is an error, not a `None`.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>> {
        let wrapped = sql::get(&self.pool, did.as_str()).await?;

        match wrapped {
            Some(wrapped) => Ok(Some(self.root.unwrap(did.as_str(), &wrapped)?)),
            None => Ok(None),
        }
    }
}
