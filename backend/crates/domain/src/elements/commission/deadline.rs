use std::str::FromStr;

use super::{CommissionId, DeadlineStatusError, LifecycleStep};
use crate::datetime::DateTimeUtc;

/// The deadline-axis Status a commission may carry — how the work stands
/// against its deadline. One nullable cell, so a set replaces; a commission with
/// no deadline never carries one.
///
/// [`Delayed`](Self::Delayed) is a manual Participant flag; [`Late`](Self::Late)
/// is the system's word, never set by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineStatus {
    /// The work is slipping — delays, but not yet lateness.
    Delayed,
    /// The deadline passed — system-set by the sweeper, never by hand.
    Late,
}

impl DeadlineStatus {
    /// Every value, in declaration order — the closed two-value vocabulary.
    pub const ALL: &[DeadlineStatus] = &[Self::Delayed, Self::Late];

    /// The stable, lowercase token written to `commission.deadline_status`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Delayed => "delayed",
            Self::Late => "late",
        }
    }
}

impl std::fmt::Display for DeadlineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Delayed => write!(f, "delayed"),
            Self::Late => write!(f, "late"),
        }
    }
}

impl TryFrom<&str> for DeadlineStatus {
    type Error = DeadlineStatusError;

    /// Resolve a stored token back to its value.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "delayed" => Self::Delayed,
            "late" => Self::Late,
            _ => return Err(DeadlineStatusError::InvalidValue),
        })
    }
}

impl FromStr for DeadlineStatus {
    type Err = DeadlineStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "delayed" => Ok(Self::Delayed),
            "late" => Ok(Self::Late),
            _ => Err(DeadlineStatusError::InvalidValue),
        }
    }
}

/// The effective deadline-axis status at `now`. `Late` is derived, never
/// persisted: a passed deadline on a non-terminal commission *is* `Late`, and it
/// supersedes a standing `Delayed` without overwriting storage. Otherwise the
/// stored manual flag. Both adapters call this while rebuilding a [`Commission`].
pub fn derive_deadline_status(
    deadline: Option<DateTimeUtc>,
    lifecycle_step: &LifecycleStep,
    stored: Option<DeadlineStatus>,
    now: DateTimeUtc,
) -> Option<DeadlineStatus> {
    // No deadline means no deadline-axis status, even if a stored `Delayed`
    // lingers — it stays dormant until a deadline is set again.
    let deadline = deadline?;
    if deadline < now && !lifecycle_step.is_terminal() {
        Some(DeadlineStatus::Late)
    } else {
        stored
    }
}

/// One commission the deadline sweep must log as Late: deadline passed,
/// lifecycle not terminal, not yet logged (the sweep dedupes on the changelog).
/// Carries what the `late` entry needs to render without joins. The sweep only
/// logs; the state itself is derived by [`derive_deadline_status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LapsedDeadline {
    /// The commission to log Late.
    pub id: CommissionId,
    /// The deadline that was missed (named in the Late entry's payload).
    pub deadline: DateTimeUtc,
    /// The standing manual flag at scan time — what the Late entry supersedes.
    pub status: Option<DeadlineStatus>,
}

#[cfg(test)]
mod tests;
