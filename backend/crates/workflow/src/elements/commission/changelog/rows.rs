#[cfg(test)]
mod tests;

use serde_json::Value;

use super::ChangelogEntryKind;
use crate::{datetime::DateTimeUtc, elements::commission::CommissionId, elements::user::UserId};

/// A changelog entry to append; the store assigns `seq` on insert. Built via
/// [`event`](Self::event) / [`system`](Self::system) / [`note`](Self::note) so
/// the actor arm is explicit, then appended on an open unit of work — an entry
/// commits atomically with the domain write it records.
#[derive(Debug)]
pub struct NewChangelogEntry {
    /// The commission whose stream this entry joins.
    pub commission_id: CommissionId,
    /// What act the entry records.
    pub kind: ChangelogEntryKind,
    /// Who did it — `None` for a system entry.
    pub actor_id: Option<UserId>,
    /// Kind-specific parameters as JSON. Must be self-sufficient to render a
    /// sentence without joins: name titles, handles and dates by value.
    pub payload: Value,
    /// Optional free text riding the entry, or the whole content of a
    /// standalone [`Note`](ChangelogEntryKind::Note) entry.
    pub note: Option<String>,
    /// When the act happened — injected, never a wall clock. Display only; the
    /// stream's order is the store-assigned `seq`.
    pub created_at: DateTimeUtc,
}

impl NewChangelogEntry {
    /// An entry for an act a participant performed. Attach free text with
    /// [`with_note`](Self::with_note).
    pub fn event(
        commission: CommissionId,
        kind: ChangelogEntryKind,
        actor: UserId,
        payload: Value,
        at: DateTimeUtc,
    ) -> Self {
        Self {
            commission_id: commission,
            kind,
            actor_id: Some(actor),
            payload,
            note: None,
            created_at: at,
        }
    }

    /// An entry for an act the system performed — no actor. The manual
    /// `Delayed` flag is a Participant [`event`](Self::event), not this.
    pub fn system(
        commission: CommissionId,
        kind: ChangelogEntryKind,
        payload: Value,
        at: DateTimeUtc,
    ) -> Self {
        Self {
            commission_id: commission,
            kind,
            actor_id: None,
            payload,
            note: None,
            created_at: at,
        }
    }

    /// A standalone free-text note by a participant: kind
    /// [`Note`](ChangelogEntryKind::Note), the text in `note`, empty payload.
    /// `text` arrives already validated non-blank.
    pub fn note(commission: CommissionId, actor: UserId, text: String, at: DateTimeUtc) -> Self {
        Self {
            commission_id: commission,
            kind: ChangelogEntryKind::Note,
            actor_id: Some(actor),
            payload: Value::Object(serde_json::Map::new()),
            note: Some(text),
            created_at: at,
        }
    }

    /// Attach free text to an event entry. A note rides its own entry and can
    /// never point at another, so reply chains stay unrepresentable.
    pub fn with_note(mut self, text: String) -> Self {
        self.note = Some(text);
        self
    }
}

/// One stored changelog entry, as read back in stream order — the
/// [`NewChangelogEntry`] envelope plus the store-assigned `seq`. Immutable: no
/// port updates or deletes one, and the pg adapter refuses `UPDATE`.
#[derive(Debug)]
pub struct ChangelogEntry {
    /// The ordering key, assigned by the store on append: a stream reads in
    /// ascending `seq`. Monotonic per stream, not gapless.
    pub seq: i64,
    /// The commission whose stream this entry belongs to.
    pub commission_id: CommissionId,
    /// What act the entry records.
    pub kind: ChangelogEntryKind,
    /// Who did it — `None` for a system entry.
    pub actor_id: Option<UserId>,
    /// Kind-specific parameters, self-sufficient to render a sentence.
    pub payload: Value,
    /// Free text riding the entry, if any.
    pub note: Option<String>,
    /// When the act happened — displayed; `seq` is the order.
    pub created_at: DateTimeUtc,
}
