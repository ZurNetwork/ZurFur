use super::LABEL_MAX_CHARS;

/// Why a string was rejected as a composition label — a [`TabName`],
/// [`SurfaceName`], [`ElementType`], or [`Band`]. One error for all four: they
/// share one validation contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompositionLabelError {
    /// Empty once trimmed.
    #[error("composition label must not be empty")]
    Empty,
    /// Longer than [`LABEL_MAX_CHARS`] after trimming.
    #[error("composition label must be at most {LABEL_MAX_CHARS} characters")]
    TooLong,
    /// Contains a control character.
    #[error("composition label must not contain control characters")]
    ControlCharacter,
}
