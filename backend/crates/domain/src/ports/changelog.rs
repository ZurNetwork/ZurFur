//! Changelog ports: the transaction-bound append surface — an entry commits
//! atomically with the domain write it records — and the pool-backed ordered
//! read. (DD 59310081)

use async_trait::async_trait;

use crate::elements::commission::{ChangelogEntry, CommissionId, NewChangelogEntry};

/// The **append** surface of the commission changelog — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.changelog()`), so an entry
/// commits atomically with the domain write it records. This is the whole write
/// vocabulary: with no update or delete anywhere, editing history is
/// unrepresentable at the port layer. (DD 59310081)
#[async_trait]
pub trait ChangelogWrites: Send {
    /// Append one entry to its commission's stream; the store assigns `seq` at
    /// insert. Readable only once the unit commits; a rollback leaves no trace.
    async fn append(&mut self, entry: &NewChangelogEntry) -> anyhow::Result<()>;
}

/// The **read** surface of the commission changelog — pool-backed and
/// non-transactional. Who may read (participants only, uniform 404 otherwise) is
/// the caller's authorization, settled before this is reached.
#[async_trait]
pub trait ChangelogStore: Send + Sync {
    /// Every entry of `commission`'s stream in stream order — ascending `seq`,
    /// never `created_at`, which is carried for display only. An unknown
    /// commission has an empty stream, not an error. Unpaginated.
    async fn entries(&self, commission: &CommissionId) -> anyhow::Result<Vec<ChangelogEntry>>;
}
