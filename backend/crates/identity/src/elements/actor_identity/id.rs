/// The app-private key of an [`ActorIdentity`] row (UUIDv7) — the anchor every
/// kind-checked actor reference FKs to.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::FromStr,
)]
pub struct ActorIdentityId(pub(super) uuid::Uuid);

#[cfg(test)]
mod tests;
