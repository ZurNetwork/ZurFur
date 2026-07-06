//! The [`FileStore`] port (ZMVP-88, ruling E13): where a commission file entry's
//! **bytes** live — a private, Index-side blob store keyed by an **opaque**
//! [`FileKey`], deliberately apart from the commission it belongs to (blobs know
//! nothing of commissions; the commission→file link is the
//! [`CommissionFile`](crate::elements::commission::CommissionFile) row).
//!
//! **Pool-backed, not a Unit-of-Work view — on purpose.** Blob bytes cannot ride a
//! Postgres transaction, and the file entry's atomicity lives elsewhere: the
//! `file_added` changelog entry and the [`CommissionFile`] row commit together in
//! the [`UnitOfWork`](crate::ports::UnitOfWork), while the blob `put` is a separate
//! step that **precedes** that unit. A unit that then rolls back leaves the blob
//! orphaned — accepted and recorded for v1 (no row ever points at it, so it is
//! never served), the same "public write is its own retryable step" posture the
//! PDS mirror uses. This is why the pg implementation is a documented exception to
//! the bare-pool-write guard.
//!
//! **v1 ships a mock/local implementation behind this port** (the ticket's AC4):
//! an in-memory map in `adapter-mem` for tests and a local `bytea`-table store in
//! `adapter-pg` for the running binary. The real blob architecture — storage,
//! limits, formats, retention, content-addressing — is the future blob-architecture
//! walkthrough's call; a swap keeps the opaque [`FileKey`]s valid as handles.

use async_trait::async_trait;

use crate::elements::commission::{FileKey, FileMetadata, StoredFile};

/// The private blob store behind a commission file entry (ZMVP-88). Keyed by an
/// **opaque** [`FileKey`] (a UUIDv7 handle, never a content-address), it holds the
/// bytes and the caller-supplied [`FileMetadata`] and knows nothing of commissions
/// — the commission link and its authorization are the caller's, upstream of this.
///
/// Pool-backed and `&self` (like [`ProfileCache`](crate::ports::ProfileCache)): the
/// blob write is a step **outside** the domain Unit of Work, never fused with it
/// (see the module docs). Every method is idempotent-friendly for retries.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// Store `bytes` under `key` with its `metadata`. Called **before** the unit of
    /// work that records the file entry, so a later rollback may orphan the blob
    /// (accepted for v1 — nothing points at an orphan). Overwriting the same key is
    /// not expected (keys are freshly minted), but an implementation should treat a
    /// repeat as an idempotent replace rather than an error.
    async fn put(&self, key: FileKey, metadata: &FileMetadata, bytes: &[u8]) -> anyhow::Result<()>;

    /// Read the bytes and metadata stored under `key`, or `None` if the store holds
    /// nothing for it. The retrieval path has already authorized the caller against
    /// the commission and confirmed the [`CommissionFile`](crate::elements::commission::CommissionFile)
    /// link, so a `None` here means the blob is missing under an existing row — an
    /// internal inconsistency, not an authorization outcome.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<StoredFile>>;

    /// Remove the bytes under `key`. Idempotent: deleting an absent key is a no-op,
    /// not an error — the shape the commission hard-delete cascade (ZMVP-66) needs
    /// when it severs a commission's blobs.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()>;
}
