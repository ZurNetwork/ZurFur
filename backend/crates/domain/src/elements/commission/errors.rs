/// Why a string was rejected as a commission title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommissionTitleError {
    /// Empty once trimmed.
    Empty,
}

impl std::fmt::Display for CommissionTitleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommissionTitleError::Empty => write!(f, "commission title must not be empty"),
        }
    }
}

impl std::error::Error for CommissionTitleError {}

/// Why a token failed to resolve to a [`LifecycleStep`]. Surfaced as an error,
/// never a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownLifecycleStep;

impl std::fmt::Display for UnknownLifecycleStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("token is not one of: draft, batched, active, completed, cancelled, disputed")
    }
}

impl std::error::Error for UnknownLifecycleStep {}

/// Why a token failed to resolve to a [`DirectionStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownDirectionStatus;

impl std::fmt::Display for UnknownDirectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "token is not one of: waiting_for_input, waiting_for_approval, changes_requested",
        )
    }
}

impl std::error::Error for UnknownDirectionStatus {}

#[derive(Debug, PartialEq, Eq)]
pub enum DeadlineStatusError {
    ParseError,
    InvalidValue,
}

impl std::fmt::Display for DeadlineStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidValue => write!(f, "Invalid value"),
            Self::ParseError => write!(f, "Parsing error"),
        }
    }
}

/// Why a token failed to resolve to a [`Visibility`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownVisibility;

impl std::fmt::Display for UnknownVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("token is not one of: private, listed, public")
    }
}

impl std::error::Error for UnknownVisibility {}
