//! [`PgKeyStore`]: the custody store for minted `did:plc` keys over `account_keys`,
//! sealed through the DID-custody handle; the probe that opens every custody row;
//! and the re-wrap that moves the 0.1 rows into the envelope. Pool-backed: a DID's
//! keys are written while it is minted, before its Account row exists.

use std::collections::BTreeSet;
use std::fmt;

use async_trait::async_trait;
use chrono::Utc;
use domain::elements::account_keys::{AccountKeys, SecretKey};
use domain::elements::custody::{
    CustodyError, CustodySubject, ForDidCustody, PartsError, SealedEnvelope,
};
use domain::elements::did::Did;
use domain::ports::{KeyStore, ScopedSealer};
use sqlx::PgPool;
use zeroize::Zeroizing;

use crate::key_vault::{LegacyOpenError, LegacyRootKey};
use crate::queries::key_store as sql;

/// The length of one key.
const KEY_BYTES: usize = 32;

/// The three keys of a DID, in role order.
const KEY_BUNDLE_BYTES: usize = 3 * KEY_BYTES;

/// The table every custody subject of this store names first.
const SUBJECT_TABLE: &[u8] = b"account_keys";

/// PostgreSQL [`KeyStore`]: seals each DID's keys through the DID-custody handle
/// into `sealed_keys`, and opens them from there only.
pub struct PgKeyStore {
    pool: PgPool,
    sealer: ScopedSealer<ForDidCustody>,
}

/// The three keys of one DID in role order (cold recovery, operational, signing),
/// wiped on drop.
pub(crate) struct KeyBundle(Zeroizing<[u8; KEY_BUNDLE_BYTES]>);

/// Why bytes are not a [`KeyBundle`]: they are not three 32-byte keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("custody keys are three 32-byte keys")]
pub(crate) struct NotAKeyBundle;

/// Why a 0.1 row was not re-sealed. Names no DID, key id or byte.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResealError {
    /// The 0.1 ciphertext doesn't open under the configured root.
    #[error("a 0.1 custody row doesn't open under the configured root")]
    LegacyUnopenable,
    /// The 0.1 ciphertext opens, but not to three 32-byte keys.
    #[error("a 0.1 custody row doesn't hold three 32-byte keys")]
    NotThreeKeys,
    /// The DID is too long to encode as a custody subject.
    #[error("a custody subject can't be encoded")]
    Subject,
    /// The custodian refused to seal or open.
    #[error("the custodian refused the re-seal")]
    Custody(#[from] CustodyError),
    /// The new envelope opens to other bytes than the 0.1 row held.
    #[error("a re-sealed custody row opens to other keys")]
    SelfCheckMismatch,
}

/// Why the re-wrap stopped; it commits nothing then.
#[derive(Debug, thiserror::Error)]
pub enum RewrapError {
    /// A row failed its re-seal.
    #[error("a custody row failed its re-wrap")]
    Reseal(#[from] ResealError),
    /// A row's guarded update changed other than one row.
    #[error("a custody row changed while it was re-wrapped")]
    RowChanged,
    /// The database failed.
    #[error("the database failed during the re-wrap")]
    Database(#[from] sqlx::Error),
}

/// What the probe found among the custody rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct CustodyProbe {
    /// Every row holding a 0.1 ciphertext.
    pub(crate) legacy_rows: usize,
    /// The rows whose 0.1 ciphertext fails the dry run or whose envelope doesn't
    /// open to three keys; a row failing both counts once.
    pub(crate) unopenable: usize,
}

impl PgKeyStore {
    /// Build the store over a connection `pool` and the handle that seals every
    /// custody record.
    pub fn new(pool: PgPool, sealer: ScopedSealer<ForDidCustody>) -> Self {
        Self { pool, sealer }
    }

    /// Dry-runs the re-wrap of every row holding a 0.1 ciphertext, and opens every
    /// envelope as a read would, both in DID order; writes nothing. Before the
    /// envelope's column exists it reads only `did` and `wrapped_keys`; no custody
    /// table reads as none.
    pub(crate) async fn probe(
        pool: &PgPool,
        legacy_root: &LegacyRootKey,
        sealer: &ScopedSealer<ForDidCustody>,
    ) -> Result<CustodyProbe, sqlx::Error> {
        if !sql::table_exists(pool).await? {
            return Ok(CustodyProbe::default());
        }
        let legacy_rows = sql::legacy_probe(pool).await?;
        let sealed_rows = if sql::sealed_column_exists(pool).await? {
            sql::sealed_probe(pool).await?
        } else {
            Vec::new()
        };
        let mut unopenable_dids = BTreeSet::new();
        for row in &legacy_rows {
            let dry_run = reseal(legacy_root, sealer, &row.did, &row.wrapped_keys).await;
            if let Err(error) = dry_run {
                tracing::debug!(did = %row.did, %error, "a custody row fails the dry run");
                unopenable_dids.insert(row.did.clone());
            }
        }
        for row in sealed_rows {
            let opened = open_sealed(sealer, &row.did, row.sealed_keys).await;
            if let Err(error) = opened {
                tracing::debug!(did = %row.did, %error, "a custody row's envelope doesn't open");
                unopenable_dids.insert(row.did);
            }
        }
        let probe = CustodyProbe {
            legacy_rows: legacy_rows.len(),
            unopenable: unopenable_dids.len(),
        };
        Ok(probe)
    }

    /// How many rows a re-wrap would seal: every 0.1 row of `probe` before the
    /// envelope's column exists, and every row without an envelope after it.
    pub(crate) async fn pending_rewraps(
        pool: &PgPool,
        probe: &CustodyProbe,
    ) -> Result<usize, sqlx::Error> {
        if !sql::table_exists(pool).await? {
            return Ok(0);
        }
        if !sql::sealed_column_exists(pool).await? {
            return Ok(probe.legacy_rows);
        }
        let unsealed = sql::count_unsealed(pool).await?;
        // A count is never negative.
        Ok(usize::try_from(unsealed).unwrap_or_default())
    }

    /// Seals every row without an envelope, in DID order, in one transaction: each
    /// is opened, sealed, re-opened and compared, then written once. Any failure
    /// commits nothing. Returns how many rows it sealed.
    pub(crate) async fn rewrap_legacy(
        pool: &PgPool,
        legacy_root: &LegacyRootKey,
        sealer: &ScopedSealer<ForDidCustody>,
    ) -> Result<usize, RewrapError> {
        let mut unit = pool.begin().await?;
        let rows = sql::legacy_unsealed(&mut *unit).await?;
        for row in &rows {
            let envelope = reseal(legacy_root, sealer, &row.did, &row.wrapped_keys)
                .await
                .inspect_err(|error| {
                    tracing::debug!(did = %row.did, %error, "a custody row fails its re-wrap");
                })?;
            let changed = sql::store_sealed(&mut *unit, &row.did, Some(envelope.as_ref())).await?;
            if changed != 1 {
                return Err(RewrapError::RowChanged);
            }
        }
        unit.commit().await?;
        Ok(rows.len())
    }
}

impl fmt::Debug for PgKeyStore {
    /// Prints the handle's purpose only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgKeyStore")
            .field("sealer", &self.sealer)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl KeyStore for PgKeyStore {
    /// Seals `keys` for `did` and inserts them into `sealed_keys`. One DID mints
    /// once, so a duplicate insert is a constraint error.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()> {
        let bundle = KeyBundle::try_from(keys)?;
        let subject = custody_subject(did.as_ref())?;
        let envelope = self.sealer.seal(&subject, bundle.expose()).await?;
        sql::put(
            &self.pool,
            did.as_ref(),
            Some(envelope.as_ref()),
            Utc::now(),
        )
        .await?;
        Ok(())
    }

    /// Opens `did`'s envelope into [`AccountKeys`], or `None` for an unknown DID. A
    /// row with no envelope yet is [`CustodyError::NotRewrapped`]; the 0.1
    /// ciphertext is never read.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>> {
        let Some(stored) = sql::get(&self.pool, did.as_ref()).await? else {
            return Ok(None);
        };
        let sealed = stored.ok_or(CustodyError::NotRewrapped)?;
        let bundle = open_sealed(&self.sealer, did.as_ref(), sealed).await?;
        Ok(Some(AccountKeys::from(&bundle)))
    }
}

impl KeyBundle {
    /// The keys' bytes, for the one use that needs them.
    pub(crate) fn expose(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl TryFrom<&[u8]> for KeyBundle {
    type Error = NotAKeyBundle;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != KEY_BUNDLE_BYTES {
            return Err(NotAKeyBundle);
        }
        let mut bundle = Zeroizing::new([0u8; KEY_BUNDLE_BYTES]);
        bundle.copy_from_slice(bytes);
        Ok(Self(bundle))
    }
}

impl TryFrom<&AccountKeys> for KeyBundle {
    type Error = NotAKeyBundle;

    fn try_from(keys: &AccountKeys) -> Result<Self, Self::Error> {
        let roles = [&keys.cold_recovery, &keys.operational, &keys.signing];
        let mut bundle = Zeroizing::new([0u8; KEY_BUNDLE_BYTES]);
        let (slots, _) = bundle.as_chunks_mut::<KEY_BYTES>();
        for (slot, secret) in slots.iter_mut().zip(roles) {
            let key = secret.expose();
            if key.len() != KEY_BYTES {
                return Err(NotAKeyBundle);
            }
            slot.copy_from_slice(key);
        }
        Ok(Self(bundle))
    }
}

impl From<&KeyBundle> for AccountKeys {
    fn from(bundle: &KeyBundle) -> Self {
        let role_key = |role: usize| {
            let start = role * KEY_BYTES;
            SecretKey::new(bundle.0[start..start + KEY_BYTES].to_vec())
        };
        Self {
            cold_recovery: role_key(0),
            operational: role_key(1),
            signing: role_key(2),
        }
    }
}

impl From<LegacyOpenError> for ResealError {
    fn from(error: LegacyOpenError) -> Self {
        match error {
            LegacyOpenError::Unopenable => Self::LegacyUnopenable,
            LegacyOpenError::NotThreeKeys => Self::NotThreeKeys,
        }
    }
}

/// What `did`'s keys are sealed to: this table, then the DID.
fn custody_subject(did: &str) -> Result<CustodySubject, PartsError> {
    let parts: [&[u8]; 2] = [SUBJECT_TABLE, did.as_bytes()];
    CustodySubject::try_from(parts.as_slice())
}

/// Opens `did`'s envelope through the DID-custody handle, bound to `did`'s subject,
/// into its three keys: the one open a read and the probe share.
async fn open_sealed(
    sealer: &ScopedSealer<ForDidCustody>,
    did: &str,
    sealed: Vec<u8>,
) -> anyhow::Result<KeyBundle> {
    let subject = custody_subject(did)?;
    let envelope = SealedEnvelope::from(sealed);
    let opened = sealer.open(&subject, &envelope).await?;
    let bundle = KeyBundle::try_from(opened.expose())?;
    Ok(bundle)
}

/// Opens a 0.1 row with the legacy root, seals its keys under DID custody, opens
/// the new envelope and compares it with the 0.1 keys: the envelope only when the
/// two match.
async fn reseal(
    legacy_root: &LegacyRootKey,
    sealer: &ScopedSealer<ForDidCustody>,
    did: &str,
    wrapped_keys: &[u8],
) -> Result<SealedEnvelope, ResealError> {
    let legacy_keys = legacy_root.open(did, wrapped_keys)?;
    let subject = custody_subject(did).map_err(|_| ResealError::Subject)?;
    let envelope = sealer.seal(&subject, legacy_keys.expose()).await?;
    let reopened = sealer.open(&subject, &envelope).await?;
    if reopened.expose() != legacy_keys.expose() {
        return Err(ResealError::SelfCheckMismatch);
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests;
