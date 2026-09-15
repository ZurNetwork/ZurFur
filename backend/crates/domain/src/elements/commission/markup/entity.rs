use crate::datetime::DateTimeUtc;
use crate::elements::commission::{CommissionId, file::FileKey};
use crate::elements::user::UserId;

use super::{Markup, MarkupKey};

/// One stored markup: a validated [`Markup`] anchored to a file entry, with the
/// Participant who drew it and when. Canonical for the geometry; written on the
/// same [`UnitOfWork`](crate::ports::UnitOfWork) as its `markup_added` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionMarkup {
    /// The markup's opaque key and this row's primary key.
    pub id: MarkupKey,
    /// The commission whose review loop the markup belongs to; every read
    /// scopes by it, so a key from another commission stays invisible.
    pub commission_id: CommissionId,
    /// The annotated file entry, enforced as a pair with the commission.
    pub file_id: FileKey,
    /// The Participant who drew it. Carries no foreign key onto the actor
    /// tables, so shared history survives a tombstone.
    pub added_by: UserId,
    /// The annotation itself, already past [`Markup::validate`]. Stored and
    /// served untransformed.
    pub markup: Markup,
    /// When the markup was drawn.
    pub created_at: DateTimeUtc,
}
