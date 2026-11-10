use super::{CommissionState, UnknownDirectionStatus};

/// The direction-axis Status a commission may carry — whose turn the work is
/// waiting on. Always set explicitly by a Participant, never by a content
/// event. One nullable column, so a set replaces and `None` means cleared.
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
#[strum(serialize_all = "snake_case", parse_err_ty = UnknownDirectionStatus, parse_err_fn = unknown_token)]
pub enum DirectionStatus {
    /// The work waits on input from the client side.
    WaitingForInput,
    /// The work waits on an approval.
    WaitingForApproval,
    /// Changes were requested on what was delivered.
    ChangesRequested,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> UnknownDirectionStatus {
    UnknownDirectionStatus
}

/// A direction status set, as its rule passed it: the status held, if any, and the one set.
/// Built only by [`set_direction`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionSet {
    from: Option<DirectionStatus>,
    to: DirectionStatus,
}

impl DirectionSet {
    /// The status held before, if any.
    pub fn before(&self) -> Option<DirectionStatus> {
        self.from
    }

    /// The status set.
    pub fn after(&self) -> DirectionStatus {
        self.to
    }
}

/// A direction status cleared, as its rule passed it: the status taken away. Built only by
/// [`clear_direction`]; it carries no new status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionCleared {
    from: DirectionStatus,
}

impl DirectionCleared {
    /// The status cleared.
    pub fn before(&self) -> DirectionStatus {
        self.from
    }
}

/// The value the store's direction write takes: the status held and the one written (`None`
/// clears). Built only from a passed set or clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionChange {
    from: Option<DirectionStatus>,
    to: Option<DirectionStatus>,
}

impl DirectionChange {
    /// The status held before, if any.
    pub fn before(&self) -> Option<DirectionStatus> {
        self.from
    }

    /// The status written, or `None` to clear it.
    pub fn after(&self) -> Option<DirectionStatus> {
        self.to
    }
}

impl From<DirectionSet> for DirectionChange {
    /// The set's write: its status in place of the one held.
    fn from(set: DirectionSet) -> Self {
        Self {
            from: set.from,
            to: Some(set.to),
        }
    }
}

impl From<DirectionCleared> for DirectionChange {
    /// The clear's write: no status in place of the one held.
    fn from(cleared: DirectionCleared) -> Self {
        Self {
            from: Some(cleared.from),
            to: None,
        }
    }
}

/// Sets `state`'s direction status to `to`; `None` when it already holds `to`. No state refuses
/// it.
pub fn set_direction(state: &CommissionState, to: DirectionStatus) -> Option<DirectionSet> {
    let from = state.direction_status();
    (from != Some(to)).then_some(DirectionSet { from, to })
}

/// Clears `state`'s direction status; `None` when it holds none. No state refuses it.
pub fn clear_direction(state: &CommissionState) -> Option<DirectionCleared> {
    state
        .direction_status()
        .map(|from| DirectionCleared { from })
}

#[cfg(test)]
mod tests;
