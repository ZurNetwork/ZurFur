use super::entity::MAX_COLUMNS_PER_WORKFLOW;

/// Why a string was rejected as a [`WorkflowName`](super::WorkflowName).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowNameError {
    /// Empty once trimmed.
    #[error("workflow name must not be empty")]
    Empty,
    /// Longer than 196 chars; carries the length.
    #[error("workflow name must be at most 196 characters; got {0}")]
    TooLong(usize),
}

/// Why a [`Workflow`] refused a column mutation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowError {
    /// The board already holds [`MAX_COLUMNS_PER_WORKFLOW`] columns.
    #[error("a workflow holds at most {MAX_COLUMNS_PER_WORKFLOW} columns")]
    TooManyColumns,
    /// A column of that exact name is already on the board.
    #[error("a column with that name is already on this workflow")]
    DuplicateColumnName,
    /// The commission is already in the column.
    #[error("that commission is already in this column")]
    DuplicateCommission,
    /// The insert index is past the end. Carries the offending index.
    #[error("index {0} is past the end of the list")]
    IndexOutOfRange(usize),
    /// No element with that identity is in the list.
    #[error("no such resource")]
    ElementNotFound,
    /// Stored column keys do not strictly ascend.
    #[error("stored column keys are out of order")]
    KeysOutOfOrder,
}

/// Why a string was rejected as a [`Position`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PositionError {
    #[error("position must not be empty")]
    Empty,
    /// A character outside `[0-9A-Za-z]`. Carries the offending char.
    #[error("position contains an invalid character {0:?}; only 0-9, A-Z and a-z are allowed")]
    InvalidDigit(char),
    /// Ends in `0`, which denotes the same key as without it.
    #[error("position must not end in '0'")]
    TrailingZero,
}
