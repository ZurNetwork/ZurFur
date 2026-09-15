use std::ops::Deref;
use std::str::FromStr;

use crate::elements::id::{IdError, parse_uuid};

/// The app-private key of a [`SeatInvitation`](super::SeatInvitation) (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeatInvitationId(uuid::Uuid);

impl SeatInvitationId {
    /// Wraps an already-minted UUIDv7; a fresh id is minted by
    /// [`SeatInvitation::issue`](super::SeatInvitation::issue).
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for SeatInvitationId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SeatInvitationId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}
