//! Attaching a Markup to a file entry (ZMVP-90; moved down from `api`,
//! ZMVP-205).
//!
//! A markup now has **two homes, written on one Unit of Work**: the
//! [`CommissionMarkup`] row is canonical for the geometry — the record a
//! per-file read returns — and the `markup_added` changelog entry stays the
//! timeline fact, carrying enough payload to render its sentence without a
//! join. Before the table existed the entry's payload *was* the markup, so
//! rendering one image's annotations meant loading the whole stream and
//! filtering client-side.
//!
//! Append-only: there is no edit and no delete. That used to come free from the
//! changelog's shape; with a table behind it, it is kept by the write port
//! exposing neither method.

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

/// `run`'s input: the annotating Participant, the commission whose review loop
/// this is, the file entry being annotated, and the already-decoded annotation.
///
/// [`markup`](Self::markup) arrives decoded but **not** validated — the shape
/// vocabulary is enforced by `serde` at the driver's boundary, the numeric
/// bounds by [`Markup::validate`] here.
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub file_key: FileKey,
    pub markup: Markup,
}

/// `run`'s output: the new markup's opaque key — the identity a changelog
/// payload could never provide, and what a future reply or resolution would
/// address.
#[derive(Debug)]
pub struct Output {
    pub id: MarkupKey,
}

/// Records one annotation on a file entry: any Participant, any time, never a
/// status side effect.
///
/// Authorizes first (a non-participant is
/// [`NotAMember`](CommissionError::NotAMember), which the driver renders as an
/// absent commission), then validates the geometry, then proves the target is a
/// file entry **of this commission** — a key from another commission is
/// [`FileNotFound`](CommissionError::FileNotFound), never an oracle. The
/// [`CommissionMarkup`] row and its `markup_added` entry commit atomically or
/// roll back together (Changelog DD D4), so a markup without its timeline fact
/// — or an entry pointing at no row — is unrepresentable.
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
