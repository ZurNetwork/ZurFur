use super::UnknownActorState;

/// An actor identity's liveness — a state on the immortal row, never a removal.
/// Identity is permanent and FK-enforced; liveness is consulted per-read.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = UnknownActorState,
    parse_err_fn = unknown_actor_state
)]
pub enum ActorState {
    /// The normal case: the actor is live.
    Active,
    /// The DID/PDS stopped resolving — the reference is kept, content absent.
    Pulled,
    /// Deleted: the identity is anonymized, the facts referencing it stay.
    Tombstoned,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_actor_state(token: &str) -> UnknownActorState {
    UnknownActorState(token.into())
}

#[cfg(test)]
mod tests;
