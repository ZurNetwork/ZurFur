use std::str::FromStr;

use crate::elements::{did::Did, id::IdError};

/// The identity of a Character: its own [`Did`]. Every actor mints a DID and
/// the DID IS the key, Characters included — there is no separate private id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::AsRef)]
#[as_ref(str)]
pub struct CharacterId(pub(super) Did);

impl CharacterId {
    /// The DID this id is: the actor key itself.
    pub fn did(&self) -> &Did {
        &self.0
    }
}

impl FromStr for CharacterId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<Did>()
            .map(Self)
            .map_err(|_| IdError::ParsingError)?;
        Ok(id)
    }
}
