//! The commission Seat: a 1:1 structural participant position — Creator,
//! Client, … — that exists before it is filled. A commission holds N Seats with
//! kinds repeating freely, and requirements ride on the vacant Seat.
//!
//! Seat is structural only: authority stays with Role, so [`SeatKind`] is an
//! open vocabulary. In the composition a Seat is an element typed
//! [`ElementType::seat`](super::ElementType::seat) with its interpreted data in
//! a satellite row keyed by the element's id.

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId,
            element::{ElementId, SurfaceAddress},
        },
        user::UserId,
    },
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// A Seat's kind — the semantic label of the position (Creator, Client, …). An
/// open vocabulary, never the `Role` enum; kinds repeat freely. Trimmed,
/// non-empty, at most [`MAX_CHARS`](Self::MAX_CHARS), no control characters.
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
    /// Empty once trimmed.
    Empty,
    /// Longer than [`SeatKind::MAX_CHARS`] after trimming.
    TooLong,
    /// Contains a control character.
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
    /// The length cap, in characters.
    pub const MAX_CHARS: usize = 64;

    /// The validated, trimmed kind as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SeatKind {
    type Error = SeatKindError;

    /// Validate and wrap a kind: trim, then reject empty, over
    /// [`MAX_CHARS`](Self::MAX_CHARS), or any control character. No vocabulary
    /// check — the enumeration is open.
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

impl std::str::FromStr for SeatKind {
    type Err = SeatKindError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for SeatKind {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A vacant Seat's free-text requirement prompt — "to apply, provide X".
/// Multi-line: newlines and tabs pass, every other control character is
/// rejected. Trimmed, non-empty, at most [`MAX_CHARS`](Self::MAX_CHARS).
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
    /// Empty once trimmed.
    Empty,
    /// Longer than [`SeatPrompt::MAX_CHARS`] after trimming.
    TooLong,
    /// Contains a control character other than newline/tab.
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
    /// The length cap, in characters.
    pub const MAX_CHARS: usize = 2000;

    /// The validated, trimmed prompt as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SeatPrompt {
    type Error = SeatPromptError;

    /// Validate and wrap a prompt: trim, then reject empty, over
    /// [`MAX_CHARS`](Self::MAX_CHARS), or a control character other than
    /// `\n`/`\r`/`\t`.
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

impl std::str::FromStr for SeatPrompt {
    type Err = SeatPromptError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for SeatPrompt {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A vacant Seat's external requirements link — e.g. a form whose responses
/// live off-platform. The same opaque-pointer contract as
/// [`ChannelPointer`](super::ChannelPointer): no scheme allowlist; trimmed,
/// non-empty, at most [`MAX_CHARS`](Self::MAX_CHARS), no control characters.
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
    /// Empty once trimmed.
    Empty,
    /// Longer than [`SeatLink::MAX_CHARS`] after trimming.
    TooLong,
    /// Contains a control character.
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
    /// The length cap, in characters.
    pub const MAX_CHARS: usize = 512;

    /// The validated, trimmed link as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SeatLink {
    type Error = SeatLinkError;

    /// Validate and wrap a link: trim, then reject empty, over
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
                StringBuilderViolation::Empty => SeatLinkError::Empty,
                StringBuilderViolation::TooLong { .. } => SeatLinkError::TooLong,
                StringBuilderViolation::ControlCharacter => SeatLinkError::ControlCharacter,
            })
    }
}

impl std::str::FromStr for SeatLink {
    type Err = SeatLinkError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for SeatLink {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A freshly declared Seat, ready to persist into a declared surface
/// ([`CommissionWrites::declare_seat`](crate::ports::CommissionWrites::declare_seat)).
/// One id, two rows: the store writes the carrying element and the seat
/// satellite atomically. Every Seat is born vacant — there is no occupant field
/// — and `position` is assigned by the store in-transaction.
#[derive(Debug)]
pub struct NewSeat {
    /// The element key (UUIDv7) — the seat's identity; the element and the
    /// satellite row share it.
    pub id: ElementId,
    /// The commission this Seat is declared on.
    pub commission_id: CommissionId,
    /// Where the carrying element sits: the (tab, surface) pair. An absent or
    /// foreign tab refuses with [`UnknownTab`](crate::ports::UnknownTab); an
    /// undeclared pair with [`UnknownSurface`](crate::ports::UnknownSurface).
    pub address: SurfaceAddress,
    /// The seat's semantic kind — open vocabulary, kinds repeat freely.
    pub kind: SeatKind,
    /// The optional free-text requirement prompt riding the vacant seat.
    pub prompt: Option<SeatPrompt>,
    /// The optional external requirements link riding the vacant seat.
    pub link: Option<SeatLink>,
    /// The acting User.
    pub created_by: UserId,
    /// When the seat was declared.
    pub created_at: DateTimeUtc,
}

impl NewSeat {
    /// A new Seat contributed at `address`, born vacant, carrying its kind and
    /// whatever requirements ride it. Mints the element id; authority, the tab's
    /// existence and the surface's declaration are settled on persist.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSeat, SeatKind, SurfaceAddress, SurfaceName, TabId},
    ///     did::Did,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let address = SurfaceAddress::new(
    ///     TabId::new(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let owner = UserId::new(Did::new("did:plc:alice".to_string()));
    /// let kind = "Creator".parse::<SeatKind>().unwrap();
    /// let seat = NewSeat::contributed_at(commission, address.clone(), kind, None, None, owner, Utc::now());
    /// assert_eq!(seat.address, address);
    /// assert_eq!(seat.kind.as_str(), "Creator");
    /// ```
    pub fn contributed_at(
        commission: CommissionId,
        address: SurfaceAddress,
        kind: SeatKind,
        prompt: Option<SeatPrompt>,
        link: Option<SeatLink>,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: ElementId::mint(),
            commission_id: commission,
            address,
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
/// interpreted satellite half; the element half lives in the loaded composition
/// under the same id. `occupant` is the whole occupancy model: one `Option`, so
/// more than one occupant is unrepresentable.
#[derive(Debug)]
pub struct Seat {
    /// The seat's identity: its carrying element's id (the satellite key).
    pub id: ElementId,
    /// The seat's semantic kind.
    pub kind: SeatKind,
    /// The free-text requirement prompt, if the vacant seat carries one.
    pub prompt: Option<SeatPrompt>,
    /// The external requirements link, if the vacant seat carries one.
    pub link: Option<SeatLink>,
    /// The single occupant slot: `None` while vacant.
    pub occupant: Option<UserId>,
}

impl Seat {
    /// Whether the seat is unoccupied.
    pub fn is_vacant(&self) -> bool {
        self.occupant.is_none()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::elements::did::Did;

    // The kind vocabulary is open: any label wraps, trimmed.
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

    // The prompt is multi-line free text: newlines/tabs pass, other control
    // characters and blank/oversized input refuse.
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

    // The link is an opaque pointer: no scheme allowlist, no control chars.
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

    // A declared seat's envelope, with no occupant field anywhere.
    #[test]
    fn a_new_seat_is_born_vacant_with_its_requirements() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let address = SurfaceAddress::new(
            super::super::element::TabId::new(uuid::Uuid::now_v7()),
            "content".parse().unwrap(),
        );
        let owner = UserId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7())));
        let kind = "Creator".parse::<SeatKind>().unwrap();
        let prompt = "Two refs, please.".parse::<SeatPrompt>().unwrap();
        let link = "https://forms.example/apply".parse::<SeatLink>().unwrap();

        let seat = NewSeat::contributed_at(
            commission,
            address.clone(),
            kind.clone(),
            Some(prompt.clone()),
            Some(link.clone()),
            owner.clone(),
            Utc::now(),
        );

        assert_eq!(seat.commission_id, commission);
        assert_eq!(seat.address, address);
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
        assert!(read.is_vacant(), "a seat is born vacant");
    }
}
