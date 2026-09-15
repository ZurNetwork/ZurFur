use super::ChannelPointer;

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
