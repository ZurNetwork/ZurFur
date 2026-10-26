#[cfg(test)]
mod tests;

use serde_json::{Value, json};

use super::{ChangelogEntryKind, OpenKind};
use crate::elements::commission::{
    CommissionId, DeadlineCleared, DeadlineSet, DelayedCleared, DelayedSet, DirectionCleared,
    DirectionSet, Dispute, DisputeClear, LateMark, LifecycleMove, LifecycleStep, MaturitySet,
};
use identity::elements::user::UserId;
use kernel::datetime::DateTimeUtc;

/// A changelog entry to append; the store assigns `seq` on insert. Its fields are private: a
/// gated kind is built only by its own constructor from the change its rule passed, an open
/// kind by [`event`](Self::event), a note by [`note`](Self::note). It is appended on an open
/// unit of work, so it commits atomically with the domain write it records.
#[derive(Debug)]
pub struct NewChangelogEntry {
    commission_id: CommissionId,
    kind: ChangelogEntryKind,
    actor_id: Option<UserId>,
    payload: Value,
    note: Option<String>,
    created_at: DateTimeUtc,
}

impl NewChangelogEntry {
    /// The commission whose stream this entry joins.
    pub fn commission_id(&self) -> CommissionId {
        self.commission_id
    }

    /// What act the entry records.
    pub fn kind(&self) -> ChangelogEntryKind {
        self.kind
    }

    /// Who did it — `None` for a system entry.
    pub fn actor_id(&self) -> Option<&UserId> {
        self.actor_id.as_ref()
    }

    /// Kind-specific parameters as JSON, enough to render the entry's sentence without joins;
    /// never the commission's title, since nothing written here can be removed.
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    /// Free text riding the entry, or the whole content of a standalone note.
    pub fn note_text(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// When the act happened — injected, never a wall clock. Display only; the stream's
    /// order is the store-assigned `seq`.
    pub fn created_at(&self) -> DateTimeUtc {
        self.created_at
    }

    /// An entry by `actor` of `kind` with `payload` and no note, the one shape every gated
    /// constructor shares.
    fn by(
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

    /// An entry for an act still recorded outside the den's write gate, with the payload its
    /// caller built. Attach free text with [`OpenChangelogEntry::with_note`].
    pub fn event(
        commission: CommissionId,
        kind: OpenKind,
        actor: UserId,
        payload: Value,
        at: DateTimeUtc,
    ) -> OpenChangelogEntry {
        let entry = Self::by(commission, kind.into(), actor, payload, at);
        OpenChangelogEntry(entry)
    }

    /// A standalone free-text note by a participant: kind
    /// [`Note`](ChangelogEntryKind::Note), the text in `note`, empty payload.
    /// `text` arrives already validated non-blank.
    pub fn note(commission: CommissionId, actor: UserId, text: String, at: DateTimeUtc) -> Self {
        Self {
            note: Some(text),
            ..Self::by(commission, ChangelogEntryKind::Note, actor, json!({}), at)
        }
    }

    /// A create: kind [`Created`](ChangelogEntryKind::Created), an empty
    /// payload and no note. A named constructor, not a std conversion, because the
    /// kind fixes the payload.
    pub fn created(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> Self {
        Self::by(
            commission,
            ChangelogEntryKind::Created,
            actor,
            json!({}),
            at,
        )
    }

    /// A rename: kind [`Renamed`](ChangelogEntryKind::Renamed), an empty
    /// payload and no note. A named constructor, not a std conversion, because the
    /// kind fixes the payload.
    pub fn renamed(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> Self {
        Self::by(
            commission,
            ChangelogEntryKind::Renamed,
            actor,
            json!({}),
            at,
        )
    }

    /// A lifecycle move: kind [`LifecycleMoved`](ChangelogEntryKind::LifecycleMoved),
    /// `{"from", "to"}` as step tokens.
    pub fn lifecycle_moved(
        commission: CommissionId,
        actor: UserId,
        moved: &LifecycleMove,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": <&'static str>::from(moved.before()),
            "to": <&'static str>::from(moved.after()),
        });
        Self::by(
            commission,
            ChangelogEntryKind::LifecycleMoved,
            actor,
            payload,
            at,
        )
    }

    /// A dispute raised: kind [`LifecycleMoved`](ChangelogEntryKind::LifecycleMoved),
    /// `{"from", "to": "disputed"}`, `from` the step it left.
    pub fn disputed(
        commission: CommissionId,
        actor: UserId,
        raised: &Dispute,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": <&'static str>::from(raised.before()),
            "to": <&'static str>::from(&LifecycleStep::Disputed),
        });
        Self::by(
            commission,
            ChangelogEntryKind::LifecycleMoved,
            actor,
            payload,
            at,
        )
    }

    /// A dispute cleared: kind [`LifecycleMoved`](ChangelogEntryKind::LifecycleMoved),
    /// `{"from": "disputed", "to"}`, `to` the step it returns to.
    pub fn dispute_cleared(
        commission: CommissionId,
        actor: UserId,
        cleared: &DisputeClear,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": <&'static str>::from(&LifecycleStep::Disputed),
            "to": <&'static str>::from(cleared.after()),
        });
        Self::by(
            commission,
            ChangelogEntryKind::LifecycleMoved,
            actor,
            payload,
            at,
        )
    }

    /// A rating given: kind [`MaturityRated`](ChangelogEntryKind::MaturityRated),
    /// `{"rating", "graphic"}`, the rating token and the Graphic flag it now carries.
    pub fn maturity_rated(
        commission: CommissionId,
        actor: UserId,
        rated: &MaturitySet,
        at: DateTimeUtc,
    ) -> Self {
        let given = rated.after();
        let payload = json!({
            "rating": <&'static str>::from(given.rating),
            "graphic": given.graphic,
        });
        Self::by(
            commission,
            ChangelogEntryKind::MaturityRated,
            actor,
            payload,
            at,
        )
    }

    /// A direction status set: kind [`StatusChanged`](ChangelogEntryKind::StatusChanged),
    /// `{"from", "to"}` as status tokens, `from` null when none was held.
    pub fn direction_set(
        commission: CommissionId,
        actor: UserId,
        set: &DirectionSet,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": set.before().map(<&'static str>::from),
            "to": <&'static str>::from(set.after()),
        });
        Self::by(
            commission,
            ChangelogEntryKind::StatusChanged,
            actor,
            payload,
            at,
        )
    }

    /// A direction status cleared: kind [`StatusChanged`](ChangelogEntryKind::StatusChanged),
    /// `{"from", "to": null}`.
    pub fn direction_cleared(
        commission: CommissionId,
        actor: UserId,
        cleared: &DirectionCleared,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": <&'static str>::from(cleared.before()),
            "to": Value::Null,
        });
        Self::by(
            commission,
            ChangelogEntryKind::StatusChanged,
            actor,
            payload,
            at,
        )
    }

    /// A deadline set: kind [`DeadlineExtended`](ChangelogEntryKind::DeadlineExtended) when it
    /// moves a held deadline later, else [`DeadlineSet`](ChangelogEntryKind::DeadlineSet);
    /// `{"from", "to"}`, `from` null when none was held.
    pub fn deadline_set(
        commission: CommissionId,
        actor: UserId,
        set: &DeadlineSet,
        at: DateTimeUtc,
    ) -> Self {
        let kind = if set.extends() {
            ChangelogEntryKind::DeadlineExtended
        } else {
            ChangelogEntryKind::DeadlineSet
        };
        let payload = json!({ "from": set.before(), "to": set.after() });
        Self::by(commission, kind, actor, payload, at)
    }

    /// A deadline cleared: kind [`DeadlineSet`](ChangelogEntryKind::DeadlineSet),
    /// `{"from", "to": null}`.
    pub fn deadline_cleared(
        commission: CommissionId,
        actor: UserId,
        cleared: &DeadlineCleared,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({ "from": cleared.before(), "to": Value::Null });
        Self::by(
            commission,
            ChangelogEntryKind::DeadlineSet,
            actor,
            payload,
            at,
        )
    }

    /// The Delayed flag raised: kind [`Delayed`](ChangelogEntryKind::Delayed),
    /// `{"from", "to": "delayed", "deadline"}`.
    pub fn delayed_set(
        commission: CommissionId,
        actor: UserId,
        set: &DelayedSet,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": set.before().map(<&'static str>::from),
            "to": <&'static str>::from(set.after()),
            "deadline": set.deadline(),
        });
        Self::by(commission, ChangelogEntryKind::Delayed, actor, payload, at)
    }

    /// The Delayed flag taken down: kind [`Delayed`](ChangelogEntryKind::Delayed),
    /// `{"from", "to": null, "deadline"}`.
    pub fn delayed_cleared(
        commission: CommissionId,
        actor: UserId,
        cleared: &DelayedCleared,
        at: DateTimeUtc,
    ) -> Self {
        let payload = json!({
            "from": cleared.before().map(<&'static str>::from),
            "to": Value::Null,
            "deadline": cleared.deadline(),
        });
        Self::by(commission, ChangelogEntryKind::Delayed, actor, payload, at)
    }

    /// The system's Late mark: kind [`Late`](ChangelogEntryKind::Late), no actor,
    /// `{"deadline", "from"}`, `from` the stored flag it supersedes.
    pub fn late(commission: CommissionId, mark: &LateMark, at: DateTimeUtc) -> Self {
        let payload = json!({
            "deadline": mark.deadline(),
            "from": mark.before().map(<&'static str>::from),
        });
        Self {
            commission_id: commission,
            kind: ChangelogEntryKind::Late,
            actor_id: None,
            payload,
            note: None,
            created_at: at,
        }
    }
}

/// An entry of an [`OpenKind`], the only kind free text may ride beside its payload. Becomes a
/// [`NewChangelogEntry`] by [`From`].
#[derive(Debug)]
pub struct OpenChangelogEntry(NewChangelogEntry);

impl OpenChangelogEntry {
    /// Attach free text to the entry. A note rides its own entry and can never point at
    /// another, so reply chains stay unrepresentable.
    pub fn with_note(self, text: String) -> Self {
        let Self(entry) = self;
        let noted = NewChangelogEntry {
            note: Some(text),
            ..entry
        };
        Self(noted)
    }
}

impl From<OpenChangelogEntry> for NewChangelogEntry {
    /// The entry, ready to append.
    fn from(open: OpenChangelogEntry) -> Self {
        open.0
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
