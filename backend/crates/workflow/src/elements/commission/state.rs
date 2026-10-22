use kernel::{datetime::DateTimeUtc, elements::maturity::Maturity};

use super::{Commission, DeadlineStatus, DirectionStatus, LifecycleStep, deadline};

/// The stored facts a rule on an existing commission reads: its lifecycle step, deadline,
/// stored Delayed flag, direction status and rating. Read with no clock; a rule that needs
/// the time takes it beside the state.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionState {
    lifecycle_step: LifecycleStep,
    deadline: Option<DateTimeUtc>,
    delayed_flag: Option<DeadlineStatus>,
    direction_status: Option<DirectionStatus>,
    maturity: Option<Maturity>,
}

impl CommissionState {
    /// The lifecycle step it holds.
    pub fn lifecycle_step(&self) -> &LifecycleStep {
        &self.lifecycle_step
    }

    /// Its deadline, if it has one.
    pub fn deadline(&self) -> Option<DateTimeUtc> {
        self.deadline
    }

    /// The stored manual flag: `Delayed` or nothing, never Late.
    pub fn delayed_flag(&self) -> Option<DeadlineStatus> {
        self.delayed_flag
    }

    /// Its direction status, if one is set.
    pub fn direction_status(&self) -> Option<DirectionStatus> {
        self.direction_status
    }

    /// Its rating, or `None` while unrated.
    pub fn maturity(&self) -> Option<Maturity> {
        self.maturity
    }

    /// Whether it stands Disputed.
    pub fn is_disputed(&self) -> bool {
        self.lifecycle_step == LifecycleStep::Disputed
    }

    /// The deadline-axis status at `now`: Late once the deadline is past on a step that isn't
    /// terminal, else the stored flag; nothing with no deadline.
    pub fn deadline_status_at(&self, now: DateTimeUtc) -> Option<DeadlineStatus> {
        deadline::status_at(self.deadline, &self.lifecycle_step, self.delayed_flag, now)
    }
}

impl From<&Commission> for CommissionState {
    /// The stored commission's facts, as it holds them.
    fn from(commission: &Commission) -> Self {
        Self {
            lifecycle_step: commission.lifecycle_step.clone(),
            deadline: commission.deadline,
            delayed_flag: commission.delayed_flag,
            direction_status: commission.direction_status,
            maturity: commission.maturity,
        }
    }
}

/// What the system's Late act reads: the commission's state and whether the store still owes
/// it a `late` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LateFacts {
    /// The commission's stored state.
    pub state: CommissionState,
    /// Whether no `late` entry has been recorded since its deadline last changed.
    pub owed: bool,
}

#[cfg(test)]
mod tests;
