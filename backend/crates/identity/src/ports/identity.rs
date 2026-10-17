use async_trait::async_trait;

use crate::elements::account_keys::AccountKeys;
use crate::elements::did::Did;
use crate::elements::handle::Handle;
use crate::elements::plc_operation::PlcOperationRecord;

/// Mints a sovereign `did:plc` for a platform-custodied entity, server-side —
/// unlike a visitor's DID, which precedes us and is only recognized. The live
/// minter signs an identity-only PLC genesis operation and persists the keys via
/// [`KeyStore`]; a stub floor is kept for tests.
#[async_trait]
pub trait DidMinter: Send + Sync {
    /// Mint a new account DID whose genesis `alsoKnownAs` is `at://<handle>`.
    /// Fallible: key generation, the [`KeyStore`] write, and the directory
    /// submission can each fail.
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did>;

    /// Mint a new DID with no alias: `alsoKnownAs` is empty. For an actor that
    /// claims a handle later through [`update_handle`](Self::update_handle).
    async fn mint_handleless(&self) -> anyhow::Result<Did>;

    /// Sign, log and submit `plc_tombstone` for `did`, chained on its latest
    /// logged operation. A public-boundary step run after the private delete has
    /// committed, never inside that transaction; retryable.
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()>;

    /// Re-point `did`'s `alsoKnownAs` to `handle`, replacing the old alias, chained
    /// on its latest logged operation. Its own retryable step, never inside a
    /// private-store transaction; idempotent by content-address, so a replay dedups
    /// on the log's unique `cid`. Fails if the DID has no custody keys or no
    /// operation to chain onto.
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()>;
}

/// The public-boundary lifecycle operations an already-minted DID runs for
/// itself; call sites go through [`Did`], never this port directly. Every call is
/// its own retryable step, reached only after the owning private-store
/// transaction has committed. Interface only for now — no adapter implements it.
#[async_trait]
pub trait DidOperations: Send + Sync {
    /// Sign and submit `plc_tombstone` for `did`, chained on its latest logged
    /// operation. Re-submittable; a failure leaves prior state intact.
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()>;

    /// Sign and submit the `alsoKnownAs` replacement pointing `did` at
    /// `handle`. Idempotent by content-address; re-submittable.
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()>;
}

/// Custody store for the private keys behind a minted `did:plc` — a private-store
/// port. Implementations must never persist key material in the clear and must
/// never log it ([`AccountKeys`] redacts its secrets in `Debug` and zeroizes them
/// on drop).
#[async_trait]
pub trait KeyStore: Send + Sync {
    /// Persist the custody keys for `did`, encrypted at rest. Called once during
    /// minting; overwrites nothing by contract — one DID mints once.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()>;

    /// Load and decrypt the custody keys for `did`, or `None` if unknown.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>>;
}

/// Append-only log of the `did:plc` operations Zurfur has submitted — a
/// private-store port. Every non-genesis operation chains onto the CID of the
/// DID's most recent one, and v1 never refetches the chain from the directory, so
/// this log is what the next operation chains onto. Records carry public material
/// only, never a key.
#[async_trait]
pub trait PlcOperationLog: Send + Sync {
    /// Record a submitted operation, in submission order. The log is never mutated
    /// or pruned; a tombstone is just the last entry.
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()>;

    /// The CID of the DID's most recent logged operation — the `prev` a new one
    /// must chain onto — or `None` if none is held.
    async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>>;

    /// The DID's most recent logged operation in full, or `None`, so a handle
    /// update can carry the prior op's public document fields forward without
    /// decrypting any key.
    async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>>;
}
