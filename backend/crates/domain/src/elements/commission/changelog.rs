//! The commission changelog: an append-only, immutable, per-commission record of
//! every domain event, and the platform's structured communication channel.
//! It is the only durable record of what happened — state is read directly
//! from its own tables, never replayed from this stream.
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
#[cfg(test)]
mod tests;

/// The kind of act a changelog entry records — the frozen entry taxonomy.
/// Variants whose emitter has not shipped yet are inert, never stored.
///
/// Each variant persists as its [`as_str`](Self::as_str) token in
/// `commission_changelog.kind` and resolves back through [`parse`](Self::parse),
/// so the enum owns the vocabulary. Renaming a token is a migration.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum ChangelogEntryKind {
    /// The commission was created — the stream's genesis entry.
    Created,
    /// The commission moved to another lifecycle step (Draft/Batched/Active/…).
    LifecycleMoved,
    /// A direction-status transition (the per-direction status between the
    /// creator and the commissioner).
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
    /// record and its facts surviving intact.
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
