use super::Markup;

/// Why a structurally well-formed [`Markup`] was rejected by
/// [`Markup::validate`] — the rules serde's shape checking cannot express.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MarkupError {
    /// A position coordinate lies outside the normalized `0..=1` space. Carries
    /// the field name and the offending value.
    #[error("coordinate {0} = {1} must be within 0..=1")]
    CoordinateOutOfRange(&'static str, f64),
    /// An extent (`r`/`w`/`h`) is not in `(0, 1]`.
    #[error("extent {0} = {1} must be > 0 and <= 1")]
    ExtentOutOfRange(&'static str, f64),
    /// A freehand stroke with fewer than
    /// [`MIN_FREEHAND_POINTS`](Markup::MIN_FREEHAND_POINTS) points.
    #[error(
        "a freehand stroke needs at least {} points",
        Markup::MIN_FREEHAND_POINTS
    )]
    TooFewPoints,
    /// A freehand stroke with more than
    /// [`MAX_FREEHAND_POINTS`](Markup::MAX_FREEHAND_POINTS) points.
    #[error(
        "a freehand stroke may carry at most {} points",
        Markup::MAX_FREEHAND_POINTS
    )]
    TooManyPoints,
    /// A text comment that is present but blank (empty or whitespace-only).
    #[error("markup text must not be blank when present")]
    TextBlank,
    /// A text comment longer than [`MAX_TEXT_CHARS`](Markup::MAX_TEXT_CHARS)
    /// characters.
    #[error("markup text must be at most {} characters", Markup::MAX_TEXT_CHARS)]
    TextTooLong,
}
