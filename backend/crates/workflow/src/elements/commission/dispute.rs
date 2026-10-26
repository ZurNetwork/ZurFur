use kernel::datetime::DateTimeUtc;

use super::{CommissionRefusal, CommissionState, LifecycleStep};

/// A commission's dispute as the store holds it while Disputed: the step it left, which a
/// clear returns it to, and when it began, where its deadline's clock stands still.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDispute {
    /// The step the commission held when the dispute was raised; never Disputed.
    pub left: LifecycleStep,
    /// When the dispute was raised.
    pub began: DateTimeUtc,
}

/// A dispute a rule passed: the step the commission held and when the dispute began. Built
/// only by [`raise_dispute`]; the value the store's dispute write takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispute {
    from: LifecycleStep,
    began: DateTimeUtc,
}

impl Dispute {
    /// The step the commission held, which a clear returns it to.
    pub fn before(&self) -> &LifecycleStep {
        &self.from
    }

    /// When the dispute began.
    pub fn began(&self) -> DateTimeUtc {
        self.began
    }
}

/// A dispute's clear a rule passed: the step the commission returns to, the one the dispute
/// left. Built only by [`clear_dispute`]; the value the store's clear takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisputeClear {
    to: LifecycleStep,
}

impl DisputeClear {
    /// The step the commission returns to.
    pub fn after(&self) -> &LifecycleStep {
        &self.to
    }
}

/// Raises a dispute on `state` at `now`, from whatever step it holds, Draft included; refused
/// while it is already Disputed.
pub fn raise_dispute(
    state: &CommissionState,
    now: DateTimeUtc,
) -> Result<Dispute, CommissionRefusal> {
    if state.is_disputed() {
        return Err(CommissionRefusal::CommissionDisputed);
    }
    let raised = Dispute {
        from: state.lifecycle_step().clone(),
        began: now,
    };
    Ok(raised)
}

/// Clears `state`'s dispute, back to exactly the step it left; refused when it isn't Disputed.
pub fn clear_dispute(state: &CommissionState) -> Result<DisputeClear, CommissionRefusal> {
    let held = state.dispute().filter(|_| state.is_disputed());
    let Some(dispute) = held else {
        return Err(CommissionRefusal::CommissionNotDisputed);
    };
    let cleared = DisputeClear {
        to: dispute.left.clone(),
    };
    Ok(cleared)
}

#[cfg(test)]
mod tests;
