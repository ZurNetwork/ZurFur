use super::ChannelPointer;

/// Why a string was rejected as a linked-channel pointer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChannelPointerError {
    /// Empty once trimmed.
    #[error("channel pointer must not be empty")]
    Empty,
    /// Longer than [`ChannelPointer::MAX_CHARS`] after trimming.
    #[error(
        "channel pointer must be at most {} characters",
        ChannelPointer::MAX_CHARS
    )]
    TooLong,
    /// Contains a control character.
    #[error("channel pointer must not contain control characters")]
    ControlCharacter,
}
