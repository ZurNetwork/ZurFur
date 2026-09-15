/// A stored invitation-state discriminant outside the three known states — a
/// schema-drift signal, not user input; carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown invitation state {0:?}")]
pub struct UnknownInvitationState(pub String);

/// Why an invitation lifecycle transition was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvitationError {
    /// The transition needs a pending invitation, but this one was already
    /// accepted or revoked.
    #[error("only a pending invitation can be revoked")]
    NotPending,
}
