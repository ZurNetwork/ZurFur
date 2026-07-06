//! Changelog ports (ZMVP-87; Changelog DD `30408741`): the append surface —
//! transaction-bound, so an entry commits **atomically with the domain write it
//! records** — and the pool-backed ordered read.

use async_trait::async_trait;

use crate::elements::commission::{ChangelogEntry, CommissionId, NewChangelogEntry};

/// The **append** surface of the commission changelog — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.changelog()`), never pool-backed:
/// the Changelog DD (D4) requires an entry to commit **atomically with the domain
/// write it records** (no dual write), and in this codebase atomic-with-domain-
/// writes is representable only as a Unit-of-Work view (DD `24150017`). This is
/// **the** emit API every event ticket wires into: the emitter performs its
/// domain write and appends its entry through the same open unit, so a commission
/// can never change without its record, nor gain a record without the change.
///
/// Append-only by construction (ZMVP-87 AC4): this trait is the changelog's
/// entire write vocabulary — there is no update or delete anywhere, so editing
/// history is unrepresentable at the port layer (the pg adapter additionally
/// refuses `UPDATE` at the database).
#[async_trait]
pub trait ChangelogWrites: Send {
    /// Append one entry to its commission's stream. The store assigns `seq` (the
    /// ordering key) at insert; the entry becomes readable only when the unit of
    /// work commits, and a rolled-back unit leaves no trace of it.
    async fn append(&mut self, entry: &NewChangelogEntry) -> anyhow::Result<()>;
}

/// The **read** surface of the commission changelog — pool-backed and
/// non-transactional, the read half the Changelog DD names `ChangelogStore` (its
/// atomicity decision forces the write half onto the Unit of Work as
/// [`ChangelogWrites`]). Who may read (participants only, uniform 404 for
/// everyone else) is the caller's authorization, settled before this is reached.
#[async_trait]
pub trait ChangelogStore: Send + Sync {
    /// Every entry of `commission`'s stream, in stream order — ascending `seq`,
    /// the explicit ordering key assigned at append (ZMVP-87 AC5; `created_at`
    /// is carried on each entry for display, not for ordering). An unknown
    /// commission has an empty stream, not an error. Unpaginated by design at
    /// this ticket: consumer cursors/pagination are ZMVP-100's job.
    async fn entries(&self, commission: CommissionId) -> anyhow::Result<Vec<ChangelogEntry>>;
}
