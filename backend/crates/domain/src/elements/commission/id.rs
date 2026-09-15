use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::elements::id::{IdError, parse_uuid};

/// The app-private key of a [`Commission`] (UUIDv7, so it sorts by creation
/// time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommissionId(uuid::Uuid);

impl CommissionId {
    /// Wraps an already-minted UUIDv7.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl From<Uuid> for CommissionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Deref for CommissionId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for CommissionId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}
