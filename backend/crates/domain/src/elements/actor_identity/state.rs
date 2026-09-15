use super::UnknownActorState;

/// An actor identity's liveness — a state on the immortal row, never a removal.
/// Identity is permanent and FK-enforced; liveness is consulted per-read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorState {
    /// The normal case: the actor is live.
    Active,
    /// The DID/PDS stopped resolving — the reference is kept, content absent.
    Pulled,
    /// Deleted: the identity is anonymized, the facts referencing it stay.
    Tombstoned,
}

impl ActorState {
    /// The stored spelling — exactly the values the schema's `CHECK` admits.
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorState::Active => "active",
            ActorState::Pulled => "pulled",
            ActorState::Tombstoned => "tombstoned",
        }
    }
}

impl TryFrom<&str> for ActorState {
    type Error = UnknownActorState;

    /// Parse the stored spelling back; an error means a corrupted row.
    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "active" => Ok(ActorState::Active),
            "pulled" => Ok(ActorState::Pulled),
            "tombstoned" => Ok(ActorState::Tombstoned),
            other => Err(UnknownActorState(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests;
