use super::{CommissionRefusal, CommissionState, UnknownLifecycleStep};

/// The single lifecycle state a commission holds. Always exactly one, moved
/// explicitly by a participant and never by a system event.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case", parse_err_ty = UnknownLifecycleStep, parse_err_fn = unknown_token)]
pub enum LifecycleStep {
    /// Just created; no facts yet, so hard delete is possible.
    Draft,
    /// Part of the workload but not active
    Batched,
    /// Selected to be worked in the batch
    Active,
    /// Approved and closed
    Completed,
    /// Cancelled by one of the parties
    Cancelled,
    /// Disputed and requiring intervention
    Disputed,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> UnknownLifecycleStep {
    UnknownLifecycleStep
}

impl LifecycleStep {
    /// Whether this state is terminal — closed work, out of scope for the
    /// deadline sweeper. [`Disputed`](Self::Disputed) is *not* terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// Whether a deadline missed on this step is swept Late: not on a terminal step, nor
    /// while [`Disputed`](Self::Disputed).
    pub fn lapses(&self) -> bool {
        !self.is_terminal() && *self != Self::Disputed
    }

    /// Every step a missed deadline doesn't lapse on, in declaration order.
    pub fn not_lapsing() -> Vec<LifecycleStep> {
        use strum::VariantArray;
        Self::VARIANTS
            .iter()
            .filter(|step| !step.lapses())
            .cloned()
            .collect()
    }
}

/// A lifecycle move a rule passed: the step held and the step moved to. Built only by
/// [`move_lifecycle`]; the value the store's lifecycle write takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleMove {
    from: LifecycleStep,
    to: LifecycleStep,
}

impl LifecycleMove {
    /// The step the commission held.
    pub fn before(&self) -> &LifecycleStep {
        &self.from
    }

    /// The step it moves to.
    pub fn after(&self) -> &LifecycleStep {
        &self.to
    }
}

/// Moves `state` to `to`: `None` when it already holds `to`; refused while Disputed, and into
/// Draft or Disputed, which no move enters.
pub fn move_lifecycle(
    state: &CommissionState,
    to: LifecycleStep,
) -> Result<Option<LifecycleMove>, CommissionRefusal> {
    if state.is_disputed() {
        return Err(CommissionRefusal::CommissionDisputed);
    }
    let from = state.lifecycle_step();
    if *from == to {
        return Ok(None);
    }
    if matches!(to, LifecycleStep::Draft | LifecycleStep::Disputed) {
        return Err(CommissionRefusal::LifecycleMoveRefused);
    }
    let checked = LifecycleMove {
        from: from.clone(),
        to,
    };
    Ok(Some(checked))
}

#[cfg(test)]
mod tests;
