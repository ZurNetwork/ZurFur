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

/// Why a commission rule refused an act. Each variant is one fixed answer; its token, from
/// [`IntoStaticStr`](strum::IntoStaticStr), is the word a log or a driver keys on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, strum::IntoStaticStr, strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum CommissionRefusal {
    /// The lifecycle move goes somewhere no move may go.
    #[error("the lifecycle move is refused")]
    LifecycleMoveRefused,
    /// The commission is Disputed, which holds what the act would change.
    #[error("the commission is disputed")]
    CommissionDisputed,
    /// A dispute's clear on a commission that isn't Disputed.
    #[error("the commission is not disputed")]
    CommissionNotDisputed,
    /// A deadline-axis act on a commission with no deadline.
    #[error("the commission has no deadline")]
    NoDeadline,
    /// The commission stands Late, the system's word, which no hand moves.
    #[error("the commission is late")]
    CommissionLate,
    /// The system owes no `late` entry: a skip, never shown to a user.
    #[error("no late entry is owed")]
    NothingOwed,
}

#[cfg(test)]
mod tests;
