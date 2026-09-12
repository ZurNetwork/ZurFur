//! The commission changelog: an append-only, immutable, per-commission record of
//! every domain event, and the platform's structured communication channel.
//! Nothing is derived from it. (DD 59310081)
//!
//! Not a chat: free text enters only as note entries, standalone or attached,
//! and an entry cannot reference another — replies are unrepresentable.
//! Conversation lives in the external [`ChannelPointer`].

use serde_json::Value;

use super::CommissionId;
use crate::{
    datetime::DateTimeUtc,
    elements::user::UserId,
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The kind of act a changelog entry records — the frozen entry taxonomy.
/// Variants whose emitter has not shipped yet are inert, never stored.
///
/// Each variant persists as its [`as_str`](Self::as_str) token in
/// `commission_changelog.kind` and resolves back through [`parse`](Self::parse),
/// so the enum owns the vocabulary. Renaming a token is a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangelogEntryKind {
    /// The commission was created — the stream's genesis entry.
    Created,
    /// The commission moved to another lifecycle step (Draft/Batched/Active/…).
    LifecycleMoved,
    /// A direction-status transition (the per-direction status of ZMVP-85).
    StatusChanged,
    /// A deadline was set (or cleared — the payload says which).
    DeadlineSet,
    /// An existing deadline was extended.
    DeadlineExtended,
    /// A Participant set or cleared the manual Delayed flag (the payload says
    /// which). Always an explicit act with an actor.
    Delayed,
    /// System entry: the commission became Late — its deadline passed. No actor.
    Late,
    /// A seat was declared on the commission.
    SeatDeclared,
    /// Someone was invited to a seat.
    SeatInvited,
    /// Someone applied for a seat.
    SeatApplied,
    /// A seat application/invitation was accepted — the seat fills.
    SeatAccepted,
    /// A seat application/invitation was declined.
    SeatDeclined,
    /// A seated participant left their seat.
    SeatLeft,
    /// A seated participant was evicted from their seat.
    SeatEvicted,
    /// A seat's visibility ceiling changed.
    CeilingChanged,
    /// A view grant was issued.
    ViewGrantIssued,
    /// A view grant was revoked.
    ViewGrantRevoked,
    /// Commission-Admin authority was granted.
    AdminGranted,
    /// Commission-Admin authority was revoked.
    AdminRevoked,
    /// Ownership of the commission was transferred.
    OwnershipTransferred,
    /// The commission was attached into a commission tree.
    TreeAttached,
    /// The commission was detached from a commission tree.
    TreeDetached,
    /// A phase was checked off.
    PhaseCheckedOff,
    /// A phase was approved by the client.
    PhaseApproved,
    /// A file entered the commission record.
    FileAdded,
    /// Markup was added over a file entry.
    MarkupAdded,
    /// An invoice was issued.
    InvoiceIssued,
    /// An invoice was voided.
    InvoiceVoided,
    /// An invoice was marked paid by the provider.
    InvoiceMarkedPaid,
    /// A payment was reported sent by the payer.
    InvoicePaymentSent,
    /// A gallery snapshot of the commission was published.
    SnapshotPublished,
    /// The owner archived the commission — soft-removed from active views, the
    /// record and its facts surviving intact. (DD 3014657)
    Archived,
    /// The owner un-archived the commission, returning it to active views.
    Unarchived,
    /// A standalone free-text note; the text rides the entry's `note` field.
    Note,
    /// The external linked channel was declared or replaced.
    ChannelLinked,
    /// The external linked channel was cleared.
    ChannelUnlinked,
}

impl ChangelogEntryKind {
    /// Every variant, in declaration order — the closed vocabulary.
    pub const ALL: &[ChangelogEntryKind] = &[
        Self::Created,
        Self::LifecycleMoved,
        Self::StatusChanged,
        Self::DeadlineSet,
        Self::DeadlineExtended,
        Self::Delayed,
        Self::Late,
        Self::SeatDeclared,
        Self::SeatInvited,
        Self::SeatApplied,
        Self::SeatAccepted,
        Self::SeatDeclined,
        Self::SeatLeft,
        Self::SeatEvicted,
        Self::CeilingChanged,
        Self::ViewGrantIssued,
        Self::ViewGrantRevoked,
        Self::AdminGranted,
        Self::AdminRevoked,
        Self::OwnershipTransferred,
        Self::TreeAttached,
        Self::TreeDetached,
        Self::PhaseCheckedOff,
        Self::PhaseApproved,
        Self::FileAdded,
        Self::MarkupAdded,
        Self::InvoiceIssued,
        Self::InvoiceVoided,
        Self::InvoiceMarkedPaid,
        Self::InvoicePaymentSent,
        Self::SnapshotPublished,
        Self::Archived,
        Self::Unarchived,
        Self::Note,
        Self::ChannelLinked,
        Self::ChannelUnlinked,
    ];

    /// The stable, lowercase token written to `commission_changelog.kind`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::LifecycleMoved => "lifecycle_moved",
            Self::StatusChanged => "status_changed",
            Self::DeadlineSet => "deadline_set",
            Self::DeadlineExtended => "deadline_extended",
            Self::Delayed => "delayed",
            Self::Late => "late",
            Self::SeatDeclared => "seat_declared",
            Self::SeatInvited => "seat_invited",
            Self::SeatApplied => "seat_applied",
            Self::SeatAccepted => "seat_accepted",
            Self::SeatDeclined => "seat_declined",
            Self::SeatLeft => "seat_left",
            Self::SeatEvicted => "seat_evicted",
            Self::CeilingChanged => "ceiling_changed",
            Self::ViewGrantIssued => "view_grant_issued",
            Self::ViewGrantRevoked => "view_grant_revoked",
            Self::AdminGranted => "admin_granted",
            Self::AdminRevoked => "admin_revoked",
            Self::OwnershipTransferred => "ownership_transferred",
            Self::TreeAttached => "tree_attached",
            Self::TreeDetached => "tree_detached",
            Self::PhaseCheckedOff => "phase_checked_off",
            Self::PhaseApproved => "phase_approved",
            Self::FileAdded => "file_added",
            Self::MarkupAdded => "markup_added",
            Self::InvoiceIssued => "invoice_issued",
            Self::InvoiceVoided => "invoice_voided",
            Self::InvoiceMarkedPaid => "invoice_marked_paid",
            Self::InvoicePaymentSent => "invoice_payment_sent",
            Self::SnapshotPublished => "snapshot_published",
            Self::Archived => "archived",
            Self::Unarchived => "unarchived",
            Self::Note => "note",
            Self::ChannelLinked => "channel_linked",
            Self::ChannelUnlinked => "channel_unlinked",
        }
    }

    /// Resolve a stored token back to its kind, or `None` for one outside the
    /// vocabulary. Callers surface `None` as an error, never a silent skip.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == token)
    }
}

/// A changelog entry to append; the store assigns `seq` on insert. Built via
/// [`event`](Self::event) / [`system`](Self::system) / [`note`](Self::note) so
/// the actor arm is explicit, then appended on an open unit of work — an entry
/// commits atomically with the domain write it records. (DD 59310081)
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

/// The commission's external linked-channel pointer — "where we talk": any URL
/// or handle, stored as raw text and rendered as an opaque pointer that never
/// auto-embeds, so no scheme allowlist is applied. Enforced here: trimmed,
/// non-empty, at most [`MAX_CHARS`](Self::MAX_CHARS), no control characters.
///
/// ```
/// use domain::elements::commission::ChannelPointer;
///
/// let url = "  https://t.me/refsheet-chat  ".parse::<ChannelPointer>().unwrap();
/// assert_eq!(url.as_str(), "https://t.me/refsheet-chat"); // trimmed
///
/// "@artist on Telegram".parse::<ChannelPointer>().unwrap(); // not a URL — fine
///
/// assert!("   ".parse::<ChannelPointer>().is_err()); // empty after trim
/// assert!("x\ny".parse::<ChannelPointer>().is_err()); // control character
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelPointer(String);

/// Why a string was rejected as a linked-channel pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelPointerError {
    /// Empty once trimmed.
    Empty,
    /// Longer than [`ChannelPointer::MAX_CHARS`] after trimming.
    TooLong,
    /// Contains a control character.
    ControlCharacter,
}

impl std::fmt::Display for ChannelPointerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelPointerError::Empty => write!(f, "channel pointer must not be empty"),
            ChannelPointerError::TooLong => write!(
                f,
                "channel pointer must be at most {} characters",
                ChannelPointer::MAX_CHARS
            ),
            ChannelPointerError::ControlCharacter => {
                write!(f, "channel pointer must not contain control characters")
            }
        }
    }
}

impl std::error::Error for ChannelPointerError {}

impl ChannelPointer {
    /// The length cap, in characters.
    pub const MAX_CHARS: usize = 512;

    /// The validated, trimmed pointer as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChannelPointer {
    type Error = ChannelPointerError;

    /// Validate and wrap a pointer: trim, then reject empty, over
    /// [`MAX_CHARS`](Self::MAX_CHARS), or any control character. Anything else
    /// — URL or not — is accepted.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(Self::MAX_CHARS)
            .no_control()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => ChannelPointerError::Empty,
                StringBuilderViolation::TooLong { .. } => ChannelPointerError::TooLong,
                StringBuilderViolation::ControlCharacter => ChannelPointerError::ControlCharacter,
            })
    }
}

impl std::str::FromStr for ChannelPointer {
    type Err = ChannelPointerError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for ChannelPointer {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::elements::did::Did;

    // The storage tokens round-trip and never collide.
    #[test]
    fn kind_tokens_round_trip_and_never_collide() {
        let mut seen = BTreeSet::new();
        for kind in ChangelogEntryKind::ALL {
            let token = kind.as_str();
            assert!(seen.insert(token), "duplicate token {token:?}");
            assert_eq!(
                ChangelogEntryKind::parse(token),
                Some(*kind),
                "token {token:?} must parse back to its kind",
            );
        }
    }

    // A token outside the vocabulary is refused, not guessed at.
    #[test]
    fn unknown_tokens_do_not_parse() {
        assert_eq!(ChangelogEntryKind::parse("placement_changed"), None);
        assert_eq!(ChangelogEntryKind::parse(""), None);
        assert_eq!(ChangelogEntryKind::parse("CREATED"), None);
    }

    // The pointer gate: trims, rejects blank/oversized/control input, and
    // applies no scheme allowlist.
    #[test]
    fn channel_pointer_validates_shape_but_not_scheme() {
        assert_eq!(
            " https://t.me/x "
                .parse::<ChannelPointer>()
                .unwrap()
                .as_str(),
            "https://t.me/x",
        );
        // No scheme allowlist — a bare handle is a fine pointer.
        assert!("@artist on Telegram".parse::<ChannelPointer>().is_ok());
        assert_eq!(
            "   ".parse::<ChannelPointer>(),
            Err(ChannelPointerError::Empty)
        );
        assert_eq!(
            ChannelPointer::try_from("x".repeat(ChannelPointer::MAX_CHARS + 1)),
            Err(ChannelPointerError::TooLong)
        );
        // Exactly at the cap is fine.
        assert!(ChannelPointer::try_from("x".repeat(ChannelPointer::MAX_CHARS)).is_ok());
        for bad in ["a\nb", "a\tb", "a\rb", "a\0b"] {
            assert_eq!(
                bad.parse::<ChannelPointer>(),
                Err(ChannelPointerError::ControlCharacter),
                "control characters are rejected: {bad:?}",
            );
        }
    }

    // System vs event constructors set the actor arm explicitly.
    #[test]
    fn constructors_set_the_actor_arm() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let actor = UserId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7())));
        let now = chrono::Utc::now();

        let event = NewChangelogEntry::event(
            commission,
            ChangelogEntryKind::Created,
            actor.clone(),
            serde_json::json!({}),
            now,
        );
        assert_eq!(event.actor_id, Some(actor.clone()));

        let system = NewChangelogEntry::system(
            commission,
            ChangelogEntryKind::Late,
            serde_json::json!({}),
            now,
        );
        assert_eq!(system.actor_id, None, "a system entry has no actor");

        let note = NewChangelogEntry::note(commission, actor.clone(), "hi".to_string(), now);
        assert!(matches!(note.kind, ChangelogEntryKind::Note));
        assert_eq!(note.note.as_deref(), Some("hi"));

        let attached = NewChangelogEntry::event(
            commission,
            ChangelogEntryKind::PhaseApproved,
            actor,
            serde_json::json!({ "phase": "lineart" }),
            now,
        )
        .with_note("love the colors!".to_string());
        assert_eq!(attached.note.as_deref(), Some("love the colors!"));
    }
}
