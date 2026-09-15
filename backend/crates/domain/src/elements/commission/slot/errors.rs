/// Why a string was rejected as a Slot title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotTitleError {
    /// Empty once trimmed.
    Empty,
}

impl std::fmt::Display for SlotTitleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotTitleError::Empty => write!(f, "slot title must not be empty"),
        }
    }
}

impl std::error::Error for SlotTitleError {}
