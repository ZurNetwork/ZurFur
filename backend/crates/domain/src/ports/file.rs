//! The [`FileStore`] port: where a commission file entry's bytes live — a
//! private, Index-side blob store keyed by an opaque [`FileKey`], knowing
//! nothing of commissions. Pool-backed and outside the Unit of Work, since blob
//! bytes cannot ride a Postgres transaction; a rollback therefore orphans the
//! blob, accepted because no row ever points at an orphan. The port speaks
//! [`tokio::io::AsyncRead`], never a buffer or an HTTP type.

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::elements::commission::{FileDownload, FileKey, FileName};

/// The private blob store behind a commission file entry, keyed by an opaque
/// [`FileKey`] (a UUIDv7 handle, never a content-address). The commission link
/// and its authorization are the caller's, upstream of this. Pool-backed and
/// `&self` — outside the Unit of Work, never fused with it.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// Drain `content` into the store under `key` with `filename` and
    /// `content_type`, persisting the finalized metadata and returning the byte
    /// count it measured. No size policy lives here — the caller caps `content`.
    /// Repeating under the same key is an idempotent replace.
    async fn put(
        &self,
        key: FileKey,
        filename: &FileName,
        content_type: &str,
        content: &mut (dyn AsyncRead + Send + Unpin),
    ) -> anyhow::Result<u64>;

    /// Stream back the metadata and bytes stored under `key`, or `None`. The
    /// caller has already authorized and confirmed the commission→file link, so a
    /// `None` here is an internal inconsistency, not an authorization outcome.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<FileDownload>>;

    /// Remove the bytes under `key`. Idempotent: deleting an absent key is a
    /// no-op, the shape the commission hard-delete cascade needs.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()>;
}
