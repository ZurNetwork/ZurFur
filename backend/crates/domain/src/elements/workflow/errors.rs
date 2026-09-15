use super::entity::MAX_COLUMNS_PER_WORKFLOW;

/// Why a [`Workflow`] refused a column mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    /// The board already holds [`MAX_COLUMNS_PER_WORKFLOW`] columns.
    TooManyColumns,
    /// A column of that exact name is already on the board.
    DuplicateColumnName,
    /// The commission is already in the column.
    DuplicateCommission,
    /// The insert index is past the end. Carries the offending index.
    IndexOutOfRange(usize),
    /// No element with that identity is in the list.
    ElementNotFound,
    /// Stored column keys do not strictly ascend.
    KeysOutOfOrder,
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::TooManyColumns => {
                write!(
                    f,
                    "a workflow holds at most {MAX_COLUMNS_PER_WORKFLOW} columns"
                )
            }
            WorkflowError::DuplicateColumnName => {
                write!(f, "a column with that name is already on this workflow")
            }
            WorkflowError::DuplicateCommission => {
                write!(f, "that commission is already in this column")
            }
            WorkflowError::IndexOutOfRange(index) => {
                write!(f, "index {index} is past the end of the list")
            }
            WorkflowError::ElementNotFound => write!(f, "no such resource"),
            WorkflowError::KeysOutOfOrder => write!(f, "stored column keys are out of order"),
        }
    }
}

impl std::error::Error for WorkflowError {}

/// Why a string was rejected as a [`Position`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PositionError {
    Empty,
    /// A character outside `[0-9A-Za-z]`. Carries the offending char.
    InvalidDigit(char),
    /// Ends in `0`, which denotes the same key as without it.
    TrailingZero,
}

impl std::fmt::Display for PositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PositionError::Empty => write!(f, "position must not be empty"),
            PositionError::InvalidDigit(c) => write!(
                f,
                "position contains an invalid character {c:?}; only 0-9, A-Z and a-z are allowed"
            ),
            PositionError::TrailingZero => write!(f, "position must not end in '0'"),
        }
    }
}

impl std::error::Error for PositionError {}
