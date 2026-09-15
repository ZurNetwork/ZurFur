use super::{SeatKind, SeatLink, SeatPrompt};

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
