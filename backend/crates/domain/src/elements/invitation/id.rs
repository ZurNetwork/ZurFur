use std::ops::Deref;
use std::str::FromStr;

use crate::elements::id::{IdError, parse_uuid};

/// The app-private key of an [`Invitation`] (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvitationId(uuid::Uuid);

impl InvitationId {
    /// Wraps an already-minted UUIDv7; a fresh id is minted by
    /// [`Invitation::issue`].
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for InvitationId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for InvitationId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}
