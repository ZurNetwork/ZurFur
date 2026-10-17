/// Why a string was rejected as a [`NonEmptyString`](super::NonEmptyString).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NonEmptyStringError {
    /// Empty once trimmed.
    #[error("text must not be empty")]
    Empty,
}
