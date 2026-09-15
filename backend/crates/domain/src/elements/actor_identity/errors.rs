/// The error a stored string that names no [`ActorKind`] parses to.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown actor kind: {0}")]
pub struct UnknownActorKind(pub String);

/// The error a stored string that names no [`ActorState`] parses to.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown actor state: {0}")]
pub struct UnknownActorState(pub String);
