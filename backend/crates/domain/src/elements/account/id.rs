use std::{ops::Deref, str::FromStr};

use serde::Deserialize;

use crate::elements::{did::Did, id::IdError};

/// An [`Account`]'s identifier: its [`Did`] — the DID is the only identifier,
/// with no separate surrogate id behind it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Did);

impl AccountId {
    /// Wraps an already-minted DID.
    pub fn new(id: Did) -> Self {
        Self(id)
    }

    /// The DID this id is: the actor key itself.
    pub fn did(&self) -> &Did {
        &self.0
    }
}

impl Deref for AccountId {
    type Target = Did;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for AccountId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<Did>()
            .map(Self)
            .map_err(|_| IdError::ParsingError)?;
        Ok(id)
    }
}
