use super::UnknownActorKind;

/// What kind of actor an identity row is — the closed vocabulary. A seated
/// Golem acts as a User, so there is no `golem` variant.
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
    parse_err_ty = UnknownActorKind,
    parse_err_fn = unknown_actor_kind
)]
pub enum ActorKind {
    User,
    Account,
    Character,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_actor_kind(token: &str) -> UnknownActorKind {
    UnknownActorKind(token.into())
}

#[cfg(test)]
mod tests;
