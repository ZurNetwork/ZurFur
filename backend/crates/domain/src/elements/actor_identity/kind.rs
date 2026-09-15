use super::UnknownActorKind;

/// What kind of actor an identity row is — the closed vocabulary. A seated
/// Golem acts as a User, so there is no `golem` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    User,
    Account,
    Character,
}

impl ActorKind {
    /// The stored spelling — exactly the values the schema's `CHECK` admits.
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorKind::User => "user",
            ActorKind::Account => "account",
            ActorKind::Character => "character",
        }
    }
}

impl TryFrom<&str> for ActorKind {
    type Error = UnknownActorKind;

    /// Parse the stored spelling back; an error means a corrupted row.
    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "user" => Ok(ActorKind::User),
            "account" => Ok(ActorKind::Account),
            "character" => Ok(ActorKind::Character),
            other => Err(UnknownActorKind(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests;
