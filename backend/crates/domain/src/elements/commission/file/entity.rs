use super::FileKey;
use crate::{
    datetime::DateTimeUtc,
    elements::{commission::CommissionId, user::UserId},
};

/// The Index-canonical record that a file entry belongs to a commission — the
/// private link the retrieval path reads to authorize a participant. Not a fact:
/// it cascades away with the commission, so a commission with only file entries
/// stays hard-deletable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionFile {
    /// The file entry's opaque key — the [`FileStore`](crate::ports::FileStore)
    /// handle and this row's primary key.
    pub id: FileKey,
    /// The commission whose review loop this entry joined.
    pub commission_id: CommissionId,
    /// The Participant who uploaded it. Carries no foreign key onto users, so
    /// shared history survives a tombstone.
    pub uploaded_by: UserId,
    /// When the entry was uploaded.
    pub created_at: DateTimeUtc,
}
