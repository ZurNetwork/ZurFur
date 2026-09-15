use crate::string_builder::{StringBuilder, StringBuilderViolation};

use super::{SeatKindError, SeatLinkError, SeatPromptError};

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

#[cfg(test)]
mod tests;
