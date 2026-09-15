/// Why a string was rejected as a commission title.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommissionTitleError {
    /// Empty once trimmed.
    #[error("commission title must not be empty")]
    Empty,
}

/// Why a token failed to resolve to a [`LifecycleStep`]. Surfaced as an error,
/// never a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: draft, batched, active, completed, cancelled, disputed")]
pub struct UnknownLifecycleStep;

/// Why a token failed to resolve to a [`DirectionStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: waiting_for_input, waiting_for_approval, changes_requested")]
pub struct UnknownDirectionStatus;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeadlineStatusError {
    #[error("Parsing error")]
    ParseError,
    #[error("Invalid value")]
    InvalidValue,
}

/// Why a token failed to resolve to a [`Visibility`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: private, listed, public")]
pub struct UnknownVisibility;
