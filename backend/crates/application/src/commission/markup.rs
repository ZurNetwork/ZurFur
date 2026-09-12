//! Attaching a Markup to a file entry. Two homes on one unit of work: the
//! [`CommissionMarkup`] row is canonical for the geometry, the `markup_added`
//! changelog entry is the timeline fact and carries enough payload to render
//! without a join. Append-only — the write port exposes no edit or delete.

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, CommissionId, CommissionMarkup, FileKey, Markup, MarkupKey,
            NewChangelogEntry,
        },
        user::UserId,
    },
};
use serde_json::json;

use crate::commission::{CommissionError, CommissionResult, Commissions};

/// The annotating Participant, the commission, the file entry and the
/// annotation. [`markup`](Self::markup) arrives decoded but not validated —
/// its numeric bounds are checked by [`Markup::validate`] here.
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub file_key: FileKey,
    pub markup: Markup,
}

/// The new markup's opaque key.
#[derive(Debug)]
pub struct Output {
    pub id: MarkupKey,
}

/// Records one annotation on a file entry: any Participant, any time, never a
/// status side effect. Authorizes first, then validates the geometry, then
/// proves the target is a file entry of this commission — a key from another
/// one is [`FileNotFound`](CommissionError::FileNotFound), never an oracle.
impl Commissions<'_> {
    pub async fn markup(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            file_key,
            markup,
        } = cmd;
        if !ports
            .commissions
            .is_participant(&commission_id, &actor_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }

        markup.validate().map_err(CommissionError::InvalidMarkup)?;

        ports
            .commissions
            .find_file(&commission_id, file_key)
            .await?
            .ok_or(CommissionError::FileNotFound)?;

        let id = MarkupKey::generate();
        let entry = NewChangelogEntry::event(
            commission_id,
            ChangelogEntryKind::MarkupAdded,
            actor_id.clone(),
            json!({
                "markup_id": *id,
                "file_id": *file_key,
                "markup": markup,
            }),
            now,
        );
        let markup = CommissionMarkup {
            id,
            commission_id,
            file_id: file_key,
            added_by: actor_id,
            markup,
            created_at: now,
        };

        let mut uow = self.ports().database.begin().await?;
        uow.commissions().add_markup(&markup).await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;

        Ok(Output { id })
    }
}
