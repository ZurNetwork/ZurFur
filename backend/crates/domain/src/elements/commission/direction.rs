use super::UnknownDirectionStatus;

/// The direction-axis Status a commission may carry — whose turn the work is
/// waiting on. Always set explicitly by a Participant, never by a content
/// event. One nullable column, so a set replaces and `None` means cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionStatus {
    /// The work waits on input from the client side.
    WaitingForInput,
    /// The work waits on an approval.
    WaitingForApproval,
    /// Changes were requested on what was delivered.
    ChangesRequested,
}

impl DirectionStatus {
    /// Every value, in declaration order — the closed three-value vocabulary.
    //FIXME: This is a disallowed pattern
    pub const ALL: &[DirectionStatus] = &[
        Self::WaitingForInput,
        Self::WaitingForApproval,
        Self::ChangesRequested,
    ];

    /// The stable, lowercase token written to `commission.direction_status`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WaitingForInput => "waiting_for_input",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::ChangesRequested => "changes_requested",
        }
    }
}

impl TryFrom<&str> for DirectionStatus {
    type Error = UnknownDirectionStatus;

    /// Resolve a stored token back to its value.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "waiting_for_input" => Self::WaitingForInput,
            "waiting_for_approval" => Self::WaitingForApproval,
            "changes_requested" => Self::ChangesRequested,
            _ => return Err(UnknownDirectionStatus),
        })
    }
}

impl std::fmt::Display for DirectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WaitingForInput => write!(f, "waiting_for_input"),
            Self::WaitingForApproval => write!(f, "waiting_for_approval"),
            Self::ChangesRequested => write!(f, "changes_requested"),
        }
    }
}

#[cfg(test)]
mod tests;
