/// A stored invitation-state discriminant outside the three known states — a
/// schema-drift signal, not user input; carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownInvitationState(pub String);

impl std::fmt::Display for UnknownInvitationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown invitation state {:?}", self.0)
    }
}

impl std::error::Error for UnknownInvitationState {}

/// Why an invitation lifecycle transition was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationError {
    /// The transition needs a pending invitation, but this one was already
    /// accepted or revoked.
    NotPending,
}

impl std::fmt::Display for InvitationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvitationError::NotPending => {
                write!(f, "only a pending invitation can be revoked")
            }
        }
    }
}

impl std::error::Error for InvitationError {}
