/// The error a stored string that names no [`ActorKind`] parses to.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownActorKind(pub String);

impl std::fmt::Display for UnknownActorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown actor kind: {}", self.0)
    }
}

impl std::error::Error for UnknownActorKind {}

/// The error a stored string that names no [`ActorState`] parses to.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownActorState(pub String);

impl std::fmt::Display for UnknownActorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown actor state: {}", self.0)
    }
}

impl std::error::Error for UnknownActorState {}
