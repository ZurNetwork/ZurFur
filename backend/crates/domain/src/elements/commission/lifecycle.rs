use super::UnknownLifecycleStep;

/// The single lifecycle state a commission holds. Always exactly one, moved
/// explicitly by a participant and never by a system event.
#[derive(Debug, Clone, PartialEq)]
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

impl LifecycleStep {
    /// Every state, in declaration order — the closed vocabulary.
    pub const ALL: &[LifecycleStep] = &[
        Self::Draft,
        Self::Batched,
        Self::Active,
        Self::Completed,
        Self::Cancelled,
        Self::Disputed,
    ];

    /// Whether this state is terminal — closed work, out of scope for the
    /// deadline sweeper. [`Disputed`](Self::Disputed) is *not* terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// The stable, lowercase token written to `commission.lifecycle`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Batched => "batched",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Disputed => "disputed",
        }
    }
}

impl TryFrom<&str> for LifecycleStep {
    type Error = UnknownLifecycleStep;

    /// Resolve a stored token back to its step.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "draft" => Self::Draft,
            "batched" => Self::Batched,
            "active" => Self::Active,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "disputed" => Self::Disputed,
            _ => return Err(UnknownLifecycleStep),
        })
    }
}

#[cfg(test)]
mod tests;
