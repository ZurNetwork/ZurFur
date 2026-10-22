use super::{
    CommissionId, CommissionRefusal, CommissionState, DeadlineStatusError, LateFacts, LifecycleStep,
};
use kernel::datetime::DateTimeUtc;

/// The deadline-axis Status a commission may carry — how the work stands
/// against its deadline. One nullable cell, so a set replaces; a commission with
/// no deadline never carries one.
///
/// [`Delayed`](Self::Delayed) is a manual Participant flag; [`Late`](Self::Late)
/// is the system's word, never set by hand.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case", parse_err_ty = DeadlineStatusError, parse_err_fn = unknown_token)]
pub enum DeadlineStatus {
    /// The work is slipping — delays, but not yet lateness.
    Delayed,
    /// The deadline passed — derived at the time asked, never stored or set by hand.
    Late,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> DeadlineStatusError {
    DeadlineStatusError::InvalidValue
}

/// One commission the deadline sweep must log as Late: deadline passed,
/// lifecycle not terminal, not yet logged (the sweep dedupes on the changelog).
/// Carries what the `late` entry needs to render without joins. The sweep only
/// logs; the state itself is derived at the time asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LapsedDeadline {
    /// The commission to log Late.
    pub id: CommissionId,
    /// The deadline that was missed (named in the Late entry's payload).
    pub deadline: DateTimeUtc,
    /// The standing manual flag at scan time — what the Late entry supersedes.
    pub status: Option<DeadlineStatus>,
}

/// The deadline-axis status at `now`: Late once `deadline` is past on a step that isn't
/// terminal, else the stored flag; nothing with no deadline, a lingering flag included.
pub(super) fn status_at(
    deadline: Option<DateTimeUtc>,
    lifecycle_step: &LifecycleStep,
    delayed_flag: Option<DeadlineStatus>,
    now: DateTimeUtc,
) -> Option<DeadlineStatus> {
    let deadline = deadline?;
    if deadline < now && !lifecycle_step.is_terminal() {
        Some(DeadlineStatus::Late)
    } else {
        delayed_flag
    }
}

/// A deadline set a rule passed: the deadline held, if any, and the one set. Built only by
/// [`schedule_deadline`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineSet {
    from: Option<DateTimeUtc>,
    to: DateTimeUtc,
}

impl DeadlineSet {
    /// The deadline held before, if any.
    pub fn before(&self) -> Option<DateTimeUtc> {
        self.from
    }

    /// The deadline set.
    pub fn after(&self) -> DateTimeUtc {
        self.to
    }

    /// Whether it moves a held deadline later: an extension, not a fresh set.
    pub fn extends(&self) -> bool {
        self.from.is_some_and(|held| self.to > held)
    }
}

/// A deadline clear a rule passed: the deadline it takes away. Built only by
/// [`clear_deadline`]; it carries no new deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineCleared {
    from: DateTimeUtc,
}

impl DeadlineCleared {
    /// The deadline cleared.
    pub fn before(&self) -> DateTimeUtc {
        self.from
    }
}

/// The value the store's deadline write takes: the deadline held and the one written
/// (`None` clears). Built only from a passed set or clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineChange {
    from: Option<DateTimeUtc>,
    to: Option<DateTimeUtc>,
}

impl DeadlineChange {
    /// The deadline held before, if any.
    pub fn before(&self) -> Option<DateTimeUtc> {
        self.from
    }

    /// The deadline written, or `None` to clear it.
    pub fn after(&self) -> Option<DateTimeUtc> {
        self.to
    }
}

impl From<DeadlineSet> for DeadlineChange {
    /// The set's write: its deadline in place of the one held.
    fn from(set: DeadlineSet) -> Self {
        Self {
            from: set.from,
            to: Some(set.to),
        }
    }
}

impl From<DeadlineCleared> for DeadlineChange {
    /// The clear's write: no deadline in place of the one held.
    fn from(cleared: DeadlineCleared) -> Self {
        Self {
            from: Some(cleared.from),
            to: None,
        }
    }
}

/// Sets `state`'s deadline to `to`: `None` when it already holds `to`; refused while Disputed.
pub fn schedule_deadline(
    state: &CommissionState,
    to: DateTimeUtc,
) -> Result<Option<DeadlineSet>, CommissionRefusal> {
    if state.is_disputed() {
        return Err(CommissionRefusal::CommissionDisputed);
    }
    let from = state.deadline();
    if from == Some(to) {
        return Ok(None);
    }
    Ok(Some(DeadlineSet { from, to }))
}

/// Clears `state`'s deadline: `None` when it holds none; refused while Disputed.
pub fn clear_deadline(
    state: &CommissionState,
) -> Result<Option<DeadlineCleared>, CommissionRefusal> {
    if state.is_disputed() {
        return Err(CommissionRefusal::CommissionDisputed);
    }
    let cleared = state.deadline().map(|from| DeadlineCleared { from });
    Ok(cleared)
}

/// A Delayed flag a rule passed: the status the act saw, the flag the row held, and the
/// deadline it stood against. Built only by [`set_delayed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedSet {
    from: Option<DeadlineStatus>,
    held: Option<DeadlineStatus>,
    deadline: Option<DateTimeUtc>,
}

impl DelayedSet {
    /// The deadline-axis status the act saw.
    pub fn before(&self) -> Option<DeadlineStatus> {
        self.from
    }

    /// The flag the row held when the rule read it.
    pub fn held(&self) -> Option<DeadlineStatus> {
        self.held
    }

    /// The status it sets: always [`Delayed`](DeadlineStatus::Delayed).
    pub fn after(&self) -> DeadlineStatus {
        DeadlineStatus::Delayed
    }

    /// The deadline the flag stands against.
    pub fn deadline(&self) -> Option<DateTimeUtc> {
        self.deadline
    }
}

/// A Delayed flag taken down, as a rule passed it: the status the act saw, the flag the row
/// held, and the deadline. Built only by [`clear_delayed`]; it carries no new status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedCleared {
    from: Option<DeadlineStatus>,
    held: Option<DeadlineStatus>,
    deadline: Option<DateTimeUtc>,
}

impl DelayedCleared {
    /// The deadline-axis status the act saw.
    pub fn before(&self) -> Option<DeadlineStatus> {
        self.from
    }

    /// The flag the row held when the rule read it.
    pub fn held(&self) -> Option<DeadlineStatus> {
        self.held
    }

    /// The deadline the flag stood against, if any.
    pub fn deadline(&self) -> Option<DateTimeUtc> {
        self.deadline
    }
}

/// The value the store's Delayed-flag write takes: the status the act saw, the flag the row
/// held (what the write compares against), the flag written (`None` clears) and the
/// deadline. Built only from a passed set or clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedChange {
    from: Option<DeadlineStatus>,
    held: Option<DeadlineStatus>,
    to: Option<DeadlineStatus>,
    deadline: Option<DateTimeUtc>,
}

impl DelayedChange {
    /// The deadline-axis status the act saw.
    pub fn before(&self) -> Option<DeadlineStatus> {
        self.from
    }

    /// The flag the row held when the rule read it: Late derived at the act's time never
    /// stands here, and a flag left without a deadline does.
    pub fn held(&self) -> Option<DeadlineStatus> {
        self.held
    }

    /// The flag written, or `None` to clear it.
    pub fn after(&self) -> Option<DeadlineStatus> {
        self.to
    }

    /// The deadline the flag stands against.
    pub fn deadline(&self) -> Option<DateTimeUtc> {
        self.deadline
    }
}

impl From<DelayedSet> for DelayedChange {
    /// The set's write: the Delayed flag.
    fn from(set: DelayedSet) -> Self {
        Self {
            from: set.from,
            held: set.held,
            to: Some(DeadlineStatus::Delayed),
            deadline: set.deadline,
        }
    }
}

impl From<DelayedCleared> for DelayedChange {
    /// The clear's write: no flag.
    fn from(cleared: DelayedCleared) -> Self {
        Self {
            from: cleared.from,
            held: cleared.held,
            to: None,
            deadline: cleared.deadline,
        }
    }
}

/// Raises the Delayed flag on `state` at `now`: `None` when it is already raised; refused while
/// Late (derived at `now`), then with no deadline. Allowed while Disputed.
pub fn set_delayed(
    state: &CommissionState,
    now: DateTimeUtc,
) -> Result<Option<DelayedSet>, CommissionRefusal> {
    let seen = state.deadline_status_at(now);
    if seen == Some(DeadlineStatus::Late) {
        return Err(CommissionRefusal::CommissionLate);
    }
    let deadline = state.deadline();
    if deadline.is_none() {
        return Err(CommissionRefusal::NoDeadline);
    }
    let held = state.delayed_flag();
    if held == Some(DeadlineStatus::Delayed) {
        return Ok(None);
    }
    Ok(Some(DelayedSet {
        from: seen,
        held,
        deadline,
    }))
}

/// Takes the Delayed flag down on `state` at `now`: `None` when none is stored; refused while
/// Late (derived at `now`). Allowed while Disputed, and with no deadline.
pub fn clear_delayed(
    state: &CommissionState,
    now: DateTimeUtc,
) -> Result<Option<DelayedCleared>, CommissionRefusal> {
    let seen = state.deadline_status_at(now);
    if seen == Some(DeadlineStatus::Late) {
        return Err(CommissionRefusal::CommissionLate);
    }
    let held = state.delayed_flag();
    if held.is_none() {
        return Ok(None);
    }
    Ok(Some(DelayedCleared {
        from: seen,
        held,
        deadline: state.deadline(),
    }))
}

/// The system's Late mark a rule passed: the deadline missed and the stored flag it
/// supersedes. Built only by [`owes_late`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateMark {
    deadline: DateTimeUtc,
    from: Option<DeadlineStatus>,
}

impl LateMark {
    /// The deadline missed.
    pub fn deadline(&self) -> DateTimeUtc {
        self.deadline
    }

    /// The stored flag the mark supersedes.
    pub fn before(&self) -> Option<DeadlineStatus> {
        self.from
    }
}

/// The `late` entry `facts` owe at `now`: owed when the deadline is strictly before `now`, the
/// step isn't terminal, the commission isn't Disputed and no entry is recorded yet.
pub fn owes_late(facts: &LateFacts, now: DateTimeUtc) -> Result<LateMark, CommissionRefusal> {
    let state = &facts.state;
    let Some(deadline) = state.deadline() else {
        return Err(CommissionRefusal::NothingOwed);
    };
    let lapsed = deadline < now && !state.lifecycle_step().is_terminal();
    if !lapsed || state.is_disputed() || !facts.owed {
        return Err(CommissionRefusal::NothingOwed);
    }
    Ok(LateMark {
        deadline,
        from: state.delayed_flag(),
    })
}

#[cfg(test)]
mod tests;
