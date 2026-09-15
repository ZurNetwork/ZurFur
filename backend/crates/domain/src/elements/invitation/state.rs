use super::UnknownInvitationState;

/// Where an invitation sits in its lifecycle. Pending from issuance until
/// accepted or revoked; both end states are terminal and there is no expiry.
/// Persisted as its lowercase [`as_str`](InvitationState::as_str) discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationState {
    /// Issued and awaiting the invited User's decision — the only state that
    /// may be revoked or accepted.
    Pending,
    /// The invited User accepted and the membership was minted. Terminal.
    Accepted,
    /// The issuer revoked the offer before it was accepted. Terminal.
    Revoked,
}

impl std::fmt::Display for InvitationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Accepted => write!(f, "accepted"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

impl TryFrom<String> for InvitationState {
    type Error = UnknownInvitationState;

    /// Parse a stored discriminant back into a state.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "pending" => Ok(InvitationState::Pending),
            "accepted" => Ok(InvitationState::Accepted),
            "revoked" => Ok(InvitationState::Revoked),
            _ => Err(UnknownInvitationState(value)),
        }
    }
}

impl InvitationState {
    /// The lowercase discriminant the store persists.
    pub fn as_str(&self) -> &'static str {
        match self {
            InvitationState::Pending => "pending",
            InvitationState::Accepted => "accepted",
            InvitationState::Revoked => "revoked",
        }
    }
}

#[cfg(test)]
mod tests;
