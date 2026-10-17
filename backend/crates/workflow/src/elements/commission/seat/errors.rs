use super::{SeatKind, SeatLink, SeatPrompt};

/// Why a string was rejected as a Seat kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SeatKindError {
    /// Empty once trimmed.
    #[error("seat kind must not be empty")]
    Empty,
    /// Longer than [`SeatKind::MAX_CHARS`] after trimming.
    #[error("seat kind must be at most {} characters", SeatKind::MAX_CHARS)]
    TooLong,
    /// Contains a control character.
    #[error("seat kind must not contain control characters")]
    ControlCharacter,
}

/// Why a string was rejected as a Seat requirement prompt.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SeatPromptError {
    /// Empty once trimmed.
    #[error("seat prompt must not be empty")]
    Empty,
    /// Longer than [`SeatPrompt::MAX_CHARS`] after trimming.
    #[error("seat prompt must be at most {} characters", SeatPrompt::MAX_CHARS)]
    TooLong,
    /// Contains a control character other than newline/tab.
    #[error("seat prompt must not contain control characters (newlines and tabs are fine)")]
    ControlCharacter,
}

/// Why a string was rejected as a Seat requirements link.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SeatLinkError {
    /// Empty once trimmed.
    #[error("seat link must not be empty")]
    Empty,
    /// Longer than [`SeatLink::MAX_CHARS`] after trimming.
    #[error("seat link must be at most {} characters", SeatLink::MAX_CHARS)]
    TooLong,
    /// Contains a control character.
    #[error("seat link must not contain control characters")]
    ControlCharacter,
}
