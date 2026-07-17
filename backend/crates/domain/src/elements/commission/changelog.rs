//! The commission **changelog** (ZMVP-87): the commission's memory — an
//! append-only, immutable, per-commission record of every domain event, and the
//! platform's structured communication channel (Changelog DD `30408741`).
//!
//! Deliberately **not a chat**: free text enters only as note entries —
//! standalone or attached to an event — with zero dialogue machinery (no replies,
//! threads, or mentions; an entry cannot reference another entry, so replies are
//! structurally unrepresentable). Conversation lives in the commission's external
//! **linked channel** ([`ChannelPointer`]), which Zurfur renders as an opaque
//! pointer and never integrates with.
//!
//! Three shapes live here:
//! - [`ChangelogEntryKind`] — the **frozen** event taxonomy (the DD's
//!   Responsibility-1 design set, plus the note and channel-pointer entries its
//!   Decisions 1–2 add). Most variants are *inert* today: their emitters land
//!   with their own tickets and this enum is their to-wire checklist.
//! - [`NewChangelogEntry`] / [`ChangelogEntry`] — the append and read shapes of
//!   one entry. The stream is ordered by the store-assigned `seq`; `created_at`
//!   is carried for display.
//! - [`ChannelPointer`] — the validated "where we talk" text.

use serde_json::Value;

use super::CommissionId;
use crate::{
    datetime::DateTimeUtc,
    elements::user::UserId,
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The kind of act a changelog entry records — the **frozen entry taxonomy** of
/// the Changelog DD (`30408741`, Responsibility 1), plus the [`Note`] entry its
/// Decision 1 admits and the channel-pointer entries its Decision 2 makes
/// changelog-recorded.
///
/// Frozen **whole** at ZMVP-87 (the DD's "exact enum at ticket time") so later
/// tickets emit *existing* variants instead of each editing this definition:
/// an unemitted variant is inert — never stored until its emitter ships — and
/// doubles as the visible to-wire checklist. The emitters: lifecycle → ZMVP-84;
/// direction status → ZMVP-85; deadline set/extend, the manual [`Delayed`]
/// flag, and the system [`Late`] → ZMVP-86; seats → ZMVP-76/78/79/80/82; ceilings →
/// ZMVP-96; view grants → ZMVP-70; Admin grant/revoke → held for the Commission
/// Admin ticket (ZMVP-83); ownership transfer → ZMVP-69; tree attach/detach —
/// commission *relationships*, outside this epic; phases → ZMVP-93/94; files →
/// ZMVP-88; markup → ZMVP-90; invoices → ZMVP-95; snapshot publish — the
/// gallery-publish unit; [`Created`], [`Note`], [`ChannelLinked`] and
/// [`ChannelUnlinked`] are emitted from ZMVP-87 itself.
///
/// **Amended once, additively** (ZMVP-68): the pre-build interview ruled that
/// un-archive exists and that archive and un-archive are **both** changelog
/// entries (Engineer ruling 2026-07-05, recorded on the ticket) — the ZMVP-87
/// taxonomy predated that ruling and carried neither, so [`Archived`] and
/// [`Unarchived`] were added and are emitted from ZMVP-68 itself. A ruling is
/// design authority; the matching Changelog DD (`30408741`) amendment is queued
/// for `/design-sync`.
///
/// [`Archived`]: Self::Archived
/// [`Unarchived`]: Self::Unarchived
///
/// Each variant persists as its stable [`as_str`](Self::as_str) token in the
/// `commission_changelog.kind` text column, validated back through
/// [`parse`](Self::parse) on read — so the enum, not the database, owns the
/// vocabulary. Renaming a token is a migration, not a free edit.
///
/// [`Delayed`]: Self::Delayed
/// [`Late`]: Self::Late
/// [`Created`]: Self::Created
/// [`Note`]: Self::Note
/// [`ChannelLinked`]: Self::ChannelLinked
/// [`ChannelUnlinked`]: Self::ChannelUnlinked
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
    /// A Participant set or cleared the manual Delayed "slipping" flag (the
    /// payload says which). An **explicit act with an actor** — Delayed is
    /// never system-set (Engineer ruling 2026-07-05, ZMVP-86; the frozen-time
    /// "system entry" gloss predated that ruling).
    Delayed,
    /// **System entry:** the commission became Late — its deadline passed (no
    /// actor; the deadline sweeper of ZMVP-86, the one place the system acts).
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
    /// Commission-Admin authority was granted (held for ZMVP-83; inert).
    AdminGranted,
    /// Commission-Admin authority was revoked (held for ZMVP-83; inert).
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
    /// record and its facts surviving intact (ZMVP-68; Deletion DD `3014657`).
    Archived,
    /// The owner un-archived the commission — an explicit act returning it to
    /// active views (ZMVP-68; Engineer ruling 2026-07-05).
    Unarchived,
    /// A standalone free-text note — speech into the record, never dialogue
    /// (DD Decision 1). The text rides the entry's `note` field.
    Note,
    /// The external linked channel was declared/replaced (DD Decision 2).
    ChannelLinked,
    /// The external linked channel was cleared.
    ChannelUnlinked,
}

impl ChangelogEntryKind {
    /// Every variant, in declaration order — the closed vocabulary. Lets tests
    /// prove the token mapping round-trips and stays collision-free, and gives
    /// future emitters one place to see what already exists.
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

    /// The stable, lowercase wire/storage token for this kind — the value the pg
    /// adapter writes to the `commission_changelog.kind` column and the API
    /// serves. Stable across releases (it is persisted), so renaming a token is
    /// a migration, not a free edit.
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

    /// Resolve a stored token back to its kind, or `None` for a token outside
    /// the closed vocabulary — which on a read path means row tampering or a
    /// missed migration, and surfaces as an error, never a silent skip.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == token)
    }
}

/// A changelog entry **to append** — everything an emitter supplies; the store
/// assigns `seq` (the ordering key) on insert. Built via the intent-named
/// constructors ([`event`](Self::event) / [`system`](Self::system) /
/// [`note`](Self::note)) so the actor arm is explicit, then persisted through
/// [`ChangelogWrites::append`](crate::ports::ChangelogWrites::append) on an open
/// unit of work — an entry commits **atomically with the domain write it
/// records** (Changelog DD D4, never a dual write).
#[derive(Debug)]
pub struct NewChangelogEntry {
    /// The commission whose stream this entry joins.
    pub commission_id: CommissionId,
    /// What act the entry records.
    pub kind: ChangelogEntryKind,
    /// Who did it — `None` for a system entry (e.g. the [`Late`] mark, which no
    /// participant performs).
    ///
    /// [`Late`]: ChangelogEntryKind::Late
    pub actor_id: Option<UserId>,
    /// Kind-specific parameters, carried as JSON. Must be **self-sufficient to
    /// render a sentence without joins** (the DD's core-renderable rule): name
    /// the things the sentence needs (a title, a handle, a deadline) by value,
    /// not by id alone.
    pub payload: Value,
    /// Optional free text riding the entry — the *attached* note of DD
    /// Decision 1 ("approval + 'love the colors!'"), or the whole content of a
    /// standalone [`Note`](ChangelogEntryKind::Note) entry.
    pub note: Option<String>,
    /// When the act happened — injected, never read from a wall clock here.
    /// Carried for display; the stream's order is the store-assigned `seq`.
    pub created_at: DateTimeUtc,
}

impl NewChangelogEntry {
    /// An entry for an act a participant performed. `payload` must render a
    /// sentence without joins (see [`payload`](Self::payload)); attach free text
    /// with [`with_note`](Self::with_note).
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

    /// An entry for an act the **system** performed — no actor (the shape the
    /// [`Late`] mark of ZMVP-86's sweeper uses; the manual [`Delayed`] flag is
    /// a Participant [`event`](Self::event), per the Engineer ruling
    /// 2026-07-05).
    ///
    /// [`Delayed`]: ChangelogEntryKind::Delayed
    /// [`Late`]: ChangelogEntryKind::Late
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

    /// A standalone free-text note by a participant (DD Decision 1): kind
    /// [`Note`](ChangelogEntryKind::Note), the text in the `note` field, an
    /// empty payload. `text` is already validated non-blank at the boundary.
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

    /// Attach free text to an event entry (the DD's "approval + 'love the
    /// colors!'"). Notes attach to the entry they ride — never to another
    /// entry, so reply chains stay unrepresentable.
    pub fn with_note(mut self, text: String) -> Self {
        self.note = Some(text);
        self
    }
}

/// One **stored** changelog entry, as read back in stream order — the
/// [`NewChangelogEntry`] envelope plus the store-assigned `seq`. Immutable by
/// construction: no port or route updates or deletes one (ZMVP-87 AC4); the pg
/// adapter additionally refuses `UPDATE` at the database.
#[derive(Debug)]
pub struct ChangelogEntry {
    /// The explicit ordering key, assigned by the store on append (a `bigserial`
    /// in pg): a commission's stream reads in ascending `seq`. Monotonic per
    /// stream, not gapless.
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

/// The commission's external **linked channel** pointer — "where we talk"
/// (Changelog DD Decision 2): any external URL or handle, stored as raw text and
/// rendered as an opaque pointer. Zurfur hosts no chat and never integrates with
/// the channel; because the pointer **never auto-embeds**, there is deliberately
/// **no scheme allowlist** — safe rendering is the frontend's job. What *is*
/// enforced at construction: the text is trimmed, non-empty, at most
/// [`MAX_CHARS`](Self::MAX_CHARS) characters, and free of control characters
/// (which have no place in a pointer and only serve header/log injection).
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
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than [`ChannelPointer::MAX_CHARS`] characters after trimming.
    TooLong,
    /// Contains a control character (newline, tab, NUL, …).
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
    /// The length cap, in characters — generous for any URL or handle, tight
    /// enough that the pointer stays a pointer rather than a message.
    pub const MAX_CHARS: usize = 512;

    /// The validated, trimmed pointer as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChannelPointer {
    type Error = ChannelPointerError;

    /// Validate and wrap a pointer: trim surrounding whitespace, then reject an
    /// empty result, one over [`MAX_CHARS`](Self::MAX_CHARS) characters, or any
    /// control character. Anything else — URL or not — is accepted: the value
    /// renders as an opaque pointer, never auto-embeds, so no scheme allowlist
    /// is applied (ZMVP-87 AC3).
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

/// The std parsing door: `"…".parse::<ChannelPointer>()?` — delegates to the
/// [`TryFrom<String>`] rules (ruling R6: `FromStr` for string parsing).
impl std::str::FromStr for ChannelPointer {
    type Err = ChannelPointerError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The std read-side view: any `impl AsRef<str>` bound accepts the newtype
/// directly (ruling R6); [`as_str`](Self::as_str) stays the explicit accessor.
impl AsRef<str> for ChannelPointer {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    // The storage tokens are a closed, collision-free vocabulary that round-trips.
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

    // The pointer gate: trims, rejects blank/oversized/control-character input,
    // and applies no scheme allowlist.
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
        let actor = UserId::new(uuid::Uuid::now_v7());
        let now = chrono::Utc::now();

        let event = NewChangelogEntry::event(
            commission,
            ChangelogEntryKind::Created,
            actor,
            serde_json::json!({}),
            now,
        );
        assert_eq!(event.actor_id, Some(actor));

        let system = NewChangelogEntry::system(
            commission,
            ChangelogEntryKind::Late,
            serde_json::json!({}),
            now,
        );
        assert_eq!(system.actor_id, None, "a system entry has no actor");

        let note = NewChangelogEntry::note(commission, actor, "hi".to_string(), now);
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
