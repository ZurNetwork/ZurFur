/// Why a string was rejected as a Slot title.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SlotTitleError {
    /// Empty once trimmed.
    #[error("slot title must not be empty")]
    Empty,
}
