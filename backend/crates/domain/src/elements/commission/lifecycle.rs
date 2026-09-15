use super::UnknownLifecycleStep;

/// The single lifecycle state a commission holds. Always exactly one, moved
/// explicitly by a participant and never by a system event.
#[derive(
    Debug,
    Clone,
    PartialEq,
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
}

#[cfg(test)]
mod tests;
