//! The [`FileStore`] port (ZMVP-88, streaming seam ZMVP-205): where a
//! commission file entry's bytes live — a private, Index-side blob store
//! keyed by an **opaque** [`FileKey`], deliberately apart from the
//! commission it belongs to (blobs know nothing of commissions; the
//! commission→file link is the
//! [`CommissionFile`](crate::elements::commission::CommissionFile) row).
//!
//! **Pool-backed, precedes the Unit of Work.** Blob bytes cannot ride a
//! Postgres transaction: `put` runs as its own step **before** the
//! transaction that records the [`CommissionFile`](crate::elements::commission::CommissionFile)
//! row and its `file_added` changelog entry
//! ([`UnitOfWork`](crate::ports::UnitOfWork)). A later rollback of that unit
//! leaves the blob orphaned — accepted for v1 (no row ever points at an
//! orphan), the same posture as the PDS mirror's "public write is its own
//! retryable step." This is why the pg implementation is a documented
//! exception to the bare-pool-write guard.
//!
//! **Streaming, not buffered.** The port speaks [`tokio::io::AsyncRead`],
//! never a full `Vec<u8>` or an axum type — the neutral vocabulary that keeps
//! `application` ignorant of HTTP. v1's adapters still buffer internally (an
//! in-memory map in `adapter-mem`, a `bytea` table in `adapter-pg` — both
//! documented exceptions); constant-memory transfer awaits the real
//! blob-architecture swap, which keeps these opaque [`FileKey`]s valid as
//! handles either way.

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::elements::commission::{FileDownload, FileKey, FileName};

/// The private blob store behind a commission file entry (ZMVP-88). Keyed by
/// an **opaque** [`FileKey`] (a UUIDv7 handle, never a content-address); it
/// knows nothing of commissions — the commission link and its authorization
/// are the caller's, upstream of this.
///
/// Pool-backed and `&self` (like [`ProfileCache`](crate::ports::ProfileCache)):
/// the blob write is a step **outside** the domain Unit of Work, never fused
/// with it (see the module docs).
#[async_trait]
pub trait FileStore: Send + Sync {
    /// Drain `content` into the store under `key`, pairing it with `filename`
    /// and `content_type`. Counts the bytes itself, persists the finalized
    /// [`FileMetadata`](crate::elements::commission::FileMetadata) (`byte_size`
    /// = the count), and returns that count. No size policy lives here — the
    /// caller caps `content` before calling. Repeating under the same freshly
    /// minted key is an idempotent replace, not an error.
    async fn put(
        &self,
        key: FileKey,
        filename: &FileName,
        content_type: &str,
        content: &mut (dyn AsyncRead + Send + Unpin),
    ) -> anyhow::Result<u64>;

    /// Stream back the metadata and bytes stored under `key`, or `None` if
    /// the store holds nothing for it. The caller has already authorized
    /// against the commission and confirmed the
    /// [`CommissionFile`](crate::elements::commission::CommissionFile) link,
    /// so a `None` here means the blob is missing under an existing row — an
    /// internal inconsistency, not an authorization outcome.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<FileDownload>>;

    /// Remove the bytes under `key`. Idempotent: deleting an absent key is a
    /// no-op, not an error — the shape the commission hard-delete cascade
    /// (ZMVP-66) needs when it severs a commission's blobs.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()>;
}
