//! The commission **Seat** (ZMVP-76; Referenceable/Slot/Seat DD `28311564`
//! Decisions 1, 3, 8): a 1:1 *structural* participant position — Creator,
//! Client, … — that exists **before** it is filled. A commission holds N Seats
//! with kinds repeating freely; requirements ("to apply, provide X") ride on
//! the vacant Seat; the vacancy itself is what Ask-for-Art publishes.
//!
//! Seat is structural **only**: Role keeps authority, aliases keep display
//! (DD Decision 3) — so [`SeatKind`] is an *open* vocabulary of its own,
//! deliberately **not** the administrative `Role` enum (or the commission-role
//! set ZMVP-83 later grants). In the content tree a Seat is a **component**
//! under a surface (the untyped v1 contract of ZMVP-72 — typing comes with the
//! catalog): the node gives tree position and visibility inheritance, while the
//! interpreted seat data — kind, requirements, the occupant — lives in a
//! satellite store row **keyed by the seat node's id**
//! ([`CommissionWrites::declare_seat`](crate::ports::CommissionWrites::declare_seat)).
//!
//! Alongside the Seat this ticket persists **participant-hood** itself (the
//! `commission_participant` membership row; Engineer ruling on ZMVP-76): the
//! owner is a permanent Participant holding no Seat, inserted at commission
//! creation and irremovable — the floor ZMVP-79's seated arm builds on.

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, NodeId},
        user::UserId,
    },
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// A Seat's **kind** — the semantic label of the position (Creator, Client, …),
/// validated on the way in.
///
/// An **open** vocabulary by design (Engineer ruling E21): kinds are free text,
/// not the administrative `Role` enum and not a closed platform list — the DD
/// keeps Seat (structural) and Role (authority) as separate axes, and kinds
/// repeat freely (two Creator seats are fine). Trimmed; must be non-empty, at
/// most [`MAX_CHARS`](Self::MAX_CHARS) characters, and free of control
/// characters (a kind is a label, not a message).
///
/// ```
/// use domain::elements::commission::SeatKind;
///
/// let kind = "  Creator  ".parse::<SeatKind>().unwrap();
/// assert_eq!(kind.as_str(), "Creator"); // trimmed
///
/// assert!("   ".parse::<SeatKind>().is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatKind(String);

/// Why a string was rejected as a Seat kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatKindError {
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than [`SeatKind::MAX_CHARS`] characters after trimming.
    TooLong,
    /// Contains a control character (newline, tab, NUL, …).
    ControlCharacter,
}

impl std::fmt::Display for SeatKindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeatKindError::Empty => write!(f, "seat kind must not be empty"),
            SeatKindError::TooLong => write!(
                f,
                "seat kind must be at most {} characters",
                SeatKind::MAX_CHARS
            ),
            SeatKindError::ControlCharacter => {
                write!(f, "seat kind must not contain control characters")
            }
        }
    }
}

impl std::error::Error for SeatKindError {}

impl SeatKind {
    /// The length cap, in characters — room for any position label, tight
    /// enough that a kind stays a label.
    pub const MAX_CHARS: usize = 64;

    /// The validated, trimmed kind as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SeatKind {
    type Error = SeatKindError;

    /// Validate and wrap a kind: trim surrounding whitespace, then reject an
    /// empty result, one over [`MAX_CHARS`](Self::MAX_CHARS) characters, or any
    /// control character. No vocabulary check — the enumeration is open.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(Self::MAX_CHARS)
            .no_control()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => SeatKindError::Empty,
                StringBuilderViolation::TooLong { .. } => SeatKindError::TooLong,
                StringBuilderViolation::ControlCharacter => SeatKindError::ControlCharacter,
            })
    }
}

/// The std parsing door: `"…".parse::<SeatKind>()?` — delegates to the
/// [`TryFrom<String>`] rules (ruling R6: `FromStr` for string parsing).
impl std::str::FromStr for SeatKind {
    type Err = SeatKindError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The std read-side view: any `impl AsRef<str>` bound accepts the newtype
/// directly (ruling R6); [`as_str`](Self::as_str) stays the explicit accessor.
impl AsRef<str> for SeatKind {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A vacant Seat's free-text requirement **prompt** — "to apply, provide X"
/// (DD Decision 8; the v1 requirement vocabulary, no form builder), validated
/// on the way in.
///
/// Multi-line free text: newlines and tabs are welcome, every *other* control
/// character is rejected. Trimmed; must be non-empty (an absent prompt is
/// `Option::None`, never an empty string) and at most
/// [`MAX_CHARS`](Self::MAX_CHARS) characters.
///
/// ```
/// use domain::elements::commission::SeatPrompt;
///
/// let prompt = "Show two refs.\nLink your portfolio.".parse::<SeatPrompt>().unwrap();
/// assert!(prompt.as_str().contains('\n')); // multi-line is fine
///
/// assert!("   ".parse::<SeatPrompt>().is_err()); // empty after trim
/// assert!("a\0b".parse::<SeatPrompt>().is_err()); // NUL is not
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatPrompt(String);

/// Why a string was rejected as a Seat requirement prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatPromptError {
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than [`SeatPrompt::MAX_CHARS`] characters after trimming.
    TooLong,
    /// Contains a control character other than newline/tab (NUL, escape, …).
    ControlCharacter,
}

impl std::fmt::Display for SeatPromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeatPromptError::Empty => write!(f, "seat prompt must not be empty"),
            SeatPromptError::TooLong => write!(
                f,
                "seat prompt must be at most {} characters",
                SeatPrompt::MAX_CHARS
            ),
            SeatPromptError::ControlCharacter => write!(
                f,
                "seat prompt must not contain control characters (newlines and tabs are fine)"
            ),
        }
    }
}

impl std::error::Error for SeatPromptError {}

impl SeatPrompt {
    /// The length cap, in characters — generous for a real ask ("provide two
    /// references and your rate"), tight enough that the prompt stays a prompt
    /// rather than hosting the application form the DD defers to a Plugin.
    pub const MAX_CHARS: usize = 2000;

    /// The validated, trimmed prompt as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SeatPrompt {
    type Error = SeatPromptError;

    /// Validate and wrap a prompt: trim surrounding whitespace, then reject an
    /// empty result, one over [`MAX_CHARS`](Self::MAX_CHARS) characters, or a
    /// control character other than `\n`/`\r`/`\t` (free text keeps its line
    /// structure; NUL and friends only serve injection).
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(Self::MAX_CHARS)
            .no_control_except(&['\n', '\r', '\t'])
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => SeatPromptError::Empty,
                StringBuilderViolation::TooLong { .. } => SeatPromptError::TooLong,
                StringBuilderViolation::ControlCharacter => SeatPromptError::ControlCharacter,
            })
    }
}

/// The std parsing door: `"…".parse::<SeatPrompt>()?` — delegates to the
/// [`TryFrom<String>`] rules (ruling R6: `FromStr` for string parsing).
impl std::str::FromStr for SeatPrompt {
    type Err = SeatPromptError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The std read-side view: any `impl AsRef<str>` bound accepts the newtype
/// directly (ruling R6); [`as_str`](Self::as_str) stays the explicit accessor.
impl AsRef<str> for SeatPrompt {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A vacant Seat's **external requirements link** (DD Decision 8) — e.g. a
/// Google Form whose responses live off-platform — validated on the way in.
///
/// The same opaque-pointer contract as the linked channel
/// ([`ChannelPointer`](super::ChannelPointer)): rendered as a pointer, never
/// auto-embedded, so deliberately **no scheme allowlist** — safe rendering is
/// the frontend's job. Enforced at construction: trimmed, non-empty, at most
/// [`MAX_CHARS`](Self::MAX_CHARS) characters, free of control characters.
///
/// ```
/// use domain::elements::commission::SeatLink;
///
/// let link = " https://forms.example/apply ".parse::<SeatLink>().unwrap();
/// assert_eq!(link.as_str(), "https://forms.example/apply"); // trimmed
///
/// assert!("x\ny".parse::<SeatLink>().is_err()); // control character
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatLink(String);

/// Why a string was rejected as a Seat requirements link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatLinkError {
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than [`SeatLink::MAX_CHARS`] characters after trimming.
    TooLong,
    /// Contains a control character (newline, tab, NUL, …).
    ControlCharacter,
}

impl std::fmt::Display for SeatLinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeatLinkError::Empty => write!(f, "seat link must not be empty"),
            SeatLinkError::TooLong => write!(
                f,
                "seat link must be at most {} characters",
                SeatLink::MAX_CHARS
            ),
            SeatLinkError::ControlCharacter => {
                write!(f, "seat link must not contain control characters")
            }
        }
    }
}

impl std::error::Error for SeatLinkError {}

impl SeatLink {
    /// The length cap, in characters — the same bound as the linked channel
    /// pointer: generous for any URL, tight enough to stay a pointer.
    pub const MAX_CHARS: usize = 512;

    /// The validated, trimmed link as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SeatLink {
    type Error = SeatLinkError;

    /// Validate and wrap a link: trim surrounding whitespace, then reject an
    /// empty result, one over [`MAX_CHARS`](Self::MAX_CHARS) characters, or any
    /// control character. Anything else — URL or not — is accepted: the value
    /// renders as an opaque pointer, never auto-embeds.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(Self::MAX_CHARS)
            .no_control()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => SeatLinkError::Empty,
                StringBuilderViolation::TooLong { .. } => SeatLinkError::TooLong,
                StringBuilderViolation::ControlCharacter => SeatLinkError::ControlCharacter,
            })
    }
}

/// The std parsing door: `"…".parse::<SeatLink>()?` — delegates to the
/// [`TryFrom<String>`] rules (ruling R6: `FromStr` for string parsing).
impl std::str::FromStr for SeatLink {
    type Err = SeatLinkError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The std read-side view: any `impl AsRef<str>` bound accepts the newtype
/// directly (ruling R6); [`as_str`](Self::as_str) stays the explicit accessor.
impl AsRef<str> for SeatLink {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A freshly declared Seat, ready to persist under an existing **surface**
/// ([`CommissionWrites::declare_seat`](crate::ports::CommissionWrites::declare_seat),
/// ZMVP-76).
///
/// Built with [`NewSeat::under`]. One id, two rows: the store persists a
/// component **node** (tree position + visibility inheritance, empty payload —
/// the untyped ZMVP-72 contract) *and* the interpreted seat satellite row
/// keyed by that same node id, atomically. There is deliberately no occupant
/// field: **every Seat is born vacant** (AC3's at-most-one occupant is a
/// single slot by construction; filling it is ZMVP-79's invitation-mediated
/// act, never part of declaration). Sibling `position` is absent as on
/// [`NewComponent`](super::NewComponent): the store assigns append order
/// in-transaction.
#[derive(Debug)]
pub struct NewSeat {
    /// The freshly minted node key (UUIDv7) — the seat's identity everywhere:
    /// the tree node and the satellite row share it.
    pub id: NodeId,
    /// The commission whose tree this grows. The store verifies `parent`
    /// belongs to this same commission.
    pub commission_id: CommissionId,
    /// The existing **surface** to grow under: the seat inherits that
    /// surface's visibility (a vacant Seat under a Description-visible surface
    /// is the published ask — AC4). The store refuses a component parent.
    pub parent: NodeId,
    /// The seat's semantic kind (Creator, Client, …) — open vocabulary, kinds
    /// repeat freely across a commission's seats.
    pub kind: SeatKind,
    /// The optional free-text requirement prompt riding the vacant seat.
    pub prompt: Option<SeatPrompt>,
    /// The optional external requirements link riding the vacant seat.
    pub link: Option<SeatLink>,
    /// The acting User (the owner; the route's authority gate settles that
    /// before this is built).
    pub created_by: UserId,
    /// When the seat was declared.
    pub created_at: DateTimeUtc,
}

impl NewSeat {
    /// A new Seat under `parent`, born **vacant**, carrying its kind and
    /// whatever requirements (prompt and/or link — both optional) ride it.
    /// Mints the node id; authority (owner-only in v1) and the
    /// parent-is-a-surface rule are the route's/store's concern, settled when
    /// this is persisted.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSeat, NodeId, SeatKind},
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let parent = NodeId::new(uuid::Uuid::now_v7());
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let kind = "Creator".parse::<SeatKind>().unwrap();
    /// let seat = NewSeat::under(commission, parent, kind, None, None, owner, Utc::now());
    /// assert_eq!(seat.parent, parent);
    /// assert_eq!(seat.kind.as_str(), "Creator");
    /// ```
    pub fn under(
        commission: CommissionId,
        parent: NodeId,
        kind: SeatKind,
        prompt: Option<SeatPrompt>,
        link: Option<SeatLink>,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: NodeId::new(uuid::Uuid::now_v7()),
            commission_id: commission,
            parent,
            kind,
            prompt,
            link,
            created_by,
            created_at: now,
        }
    }
}

/// One stored Seat as read back
/// ([`CommissionStore::seats`](crate::ports::CommissionStore::seats)) — the
/// interpreted satellite half; the node half (tree position, creator, instant,
/// visibility inheritance) lives in the loaded tree under the same id.
///
/// This is the **projection hook** for ZMVP-76 AC4: the viewer projection
/// (ZMVP-75, not in this lineage yet) joins these rows against the projected
/// tree by node id to render a vacant Seat under Description-visible surfaces
/// as the published ask. `occupant` is the whole occupancy model: a single
/// `Option` — at most one occupant is unrepresentable to violate (AC3) — and
/// `None` from declaration until ZMVP-79 seats someone.
#[derive(Debug)]
pub struct Seat {
    /// The seat's identity: its tree node's id (the satellite key).
    pub id: NodeId,
    /// The seat's semantic kind (open vocabulary; kinds repeat freely).
    pub kind: SeatKind,
    /// The free-text requirement prompt, if the vacant seat carries one.
    pub prompt: Option<SeatPrompt>,
    /// The external requirements link, if the vacant seat carries one.
    pub link: Option<SeatLink>,
    /// The single occupant slot: `None` while vacant (every seat from
    /// declaration), `Some` once ZMVP-79's accepted invitation fills it.
    pub occupant: Option<UserId>,
}

impl Seat {
    /// Whether the seat is unoccupied — the predicate the ask projection (AC4)
    /// and the fill guards of ZMVP-78/79/80 share.
    pub fn is_vacant(&self) -> bool {
        self.occupant.is_none()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    // AC1 — the kind vocabulary is OPEN (ruling E21): any reasonable label
    // wraps, trimmed; it is deliberately not the Role enum, so nothing here
    // checks a vocabulary.
    #[test]
    fn seat_kind_is_an_open_trimmed_vocabulary() {
        assert_eq!(
            "  Creator ".parse::<SeatKind>().unwrap().as_str(),
            "Creator"
        );
        // Not a Role, not a closed list — arbitrary labels are fine.
        assert!("Background artist".parse::<SeatKind>().is_ok());
        assert!("客户".parse::<SeatKind>().is_ok());

        assert_eq!("   ".parse::<SeatKind>(), Err(SeatKindError::Empty));
        assert_eq!(
            SeatKind::try_from("x".repeat(SeatKind::MAX_CHARS + 1)),
            Err(SeatKindError::TooLong)
        );
        assert!(SeatKind::try_from("x".repeat(SeatKind::MAX_CHARS)).is_ok());
        assert_eq!(
            "a\nb".parse::<SeatKind>(),
            Err(SeatKindError::ControlCharacter)
        );
    }

    // AC2 — the prompt is multi-line free text: newlines/tabs pass, other
    // control characters and blank/oversized input refuse.
    #[test]
    fn seat_prompt_allows_lines_but_not_injection() {
        let prompt = " Provide:\n\t- two refs\n\t- your rate "
            .parse::<SeatPrompt>()
            .unwrap();
        assert_eq!(prompt.as_str(), "Provide:\n\t- two refs\n\t- your rate");

        assert_eq!("   ".parse::<SeatPrompt>(), Err(SeatPromptError::Empty));
        assert_eq!(
            SeatPrompt::try_from("x".repeat(SeatPrompt::MAX_CHARS + 1)),
            Err(SeatPromptError::TooLong)
        );
        assert!(SeatPrompt::try_from("x".repeat(SeatPrompt::MAX_CHARS)).is_ok());
        assert_eq!(
            "a\0b".parse::<SeatPrompt>(),
            Err(SeatPromptError::ControlCharacter)
        );
        assert_eq!(
            "a\u{1b}b".parse::<SeatPrompt>(),
            Err(SeatPromptError::ControlCharacter)
        );
    }

    // AC2 — the link is an opaque pointer with the ChannelPointer contract:
    // no scheme allowlist, no control characters.
    #[test]
    fn seat_link_validates_shape_but_not_scheme() {
        assert_eq!(
            " https://forms.example/apply "
                .parse::<SeatLink>()
                .unwrap()
                .as_str(),
            "https://forms.example/apply"
        );
        // No scheme allowlist — a bare pointer is fine.
        assert!("form on my carrd".parse::<SeatLink>().is_ok());

        assert_eq!("   ".parse::<SeatLink>(), Err(SeatLinkError::Empty));
        assert_eq!(
            SeatLink::try_from("x".repeat(SeatLink::MAX_CHARS + 1)),
            Err(SeatLinkError::TooLong)
        );
        assert_eq!(
            "a\tb".parse::<SeatLink>(),
            Err(SeatLinkError::ControlCharacter)
        );
    }

    // AC1/AC3 — a declared seat's envelope: fresh id, the parent surface, the
    // acting user, its kind and requirements — and NO occupant field anywhere
    // (born vacant by construction).
    #[test]
    fn a_new_seat_is_born_vacant_with_its_requirements() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let parent = NodeId::new(uuid::Uuid::now_v7());
        let owner = UserId::new(uuid::Uuid::now_v7());
        let kind = "Creator".parse::<SeatKind>().unwrap();
        let prompt = "Two refs, please.".parse::<SeatPrompt>().unwrap();
        let link = "https://forms.example/apply".parse::<SeatLink>().unwrap();

        let seat = NewSeat::under(
            commission,
            parent,
            kind.clone(),
            Some(prompt.clone()),
            Some(link.clone()),
            owner,
            Utc::now(),
        );

        assert_eq!(seat.commission_id, commission);
        assert_eq!(seat.parent, parent);
        assert_eq!(seat.kind, kind);
        assert_eq!(seat.prompt, Some(prompt));
        assert_eq!(seat.link, Some(link));
        assert_eq!(seat.created_by, owner);
        // The read shape's single occupant slot is the whole occupancy model.
        let read = Seat {
            id: seat.id,
            kind: seat.kind.clone(),
            prompt: seat.prompt.clone(),
            link: seat.link.clone(),
            occupant: None,
        };
        assert!(read.is_vacant(), "a seat is born vacant (AC3)");
    }
}
