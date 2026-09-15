use std::ops::Deref;
use std::str::FromStr;

use crate::elements::id::{IdError, parse_uuid};

/// The app-private key of an [`ActorIdentity`] row (UUIDv7) — the anchor every
/// kind-checked actor reference FKs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorIdentityId(pub(super) uuid::Uuid);

impl ActorIdentityId {
    /// Rebuilds an id from its stored UUID; a fresh one is minted by
    /// [`ActorIdentity::mint`].
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for ActorIdentityId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for ActorIdentityId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

#[cfg(test)]
mod tests;
