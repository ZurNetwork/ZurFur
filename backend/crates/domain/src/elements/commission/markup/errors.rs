use super::Markup;

/// Why a structurally well-formed [`Markup`] was rejected by
/// [`Markup::validate`] — the rules serde's shape checking cannot express.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkupError {
    /// A position coordinate lies outside the normalized `0..=1` space. Carries
    /// the field name and the offending value.
    CoordinateOutOfRange(&'static str, f64),
    /// An extent (`r`/`w`/`h`) is not in `(0, 1]`.
    ExtentOutOfRange(&'static str, f64),
    /// A freehand stroke with fewer than
    /// [`MIN_FREEHAND_POINTS`](Markup::MIN_FREEHAND_POINTS) points.
    TooFewPoints,
    /// A freehand stroke with more than
    /// [`MAX_FREEHAND_POINTS`](Markup::MAX_FREEHAND_POINTS) points.
    TooManyPoints,
    /// A text comment that is present but blank (empty or whitespace-only).
    TextBlank,
    /// A text comment longer than [`MAX_TEXT_CHARS`](Markup::MAX_TEXT_CHARS)
    /// characters.
    TextTooLong,
}

impl std::fmt::Display for MarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarkupError::CoordinateOutOfRange(field, value) => {
                write!(f, "coordinate {field} = {value} must be within 0..=1")
            }
            MarkupError::ExtentOutOfRange(field, value) => {
                write!(f, "extent {field} = {value} must be > 0 and <= 1")
            }
            MarkupError::TooFewPoints => write!(
                f,
                "a freehand stroke needs at least {} points",
                Markup::MIN_FREEHAND_POINTS
            ),
            MarkupError::TooManyPoints => write!(
                f,
                "a freehand stroke may carry at most {} points",
                Markup::MAX_FREEHAND_POINTS
            ),
            MarkupError::TextBlank => write!(f, "markup text must not be blank when present"),
            MarkupError::TextTooLong => write!(
                f,
                "markup text must be at most {} characters",
                Markup::MAX_TEXT_CHARS
            ),
        }
    }
}

impl std::error::Error for MarkupError {}
