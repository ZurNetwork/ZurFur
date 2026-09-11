//! [`CharacterId`] — the identity of a Character. **Stub.**
//!
//! A Character is a repository and representation of a character, kept by one or
//! more Keepers and identified by its own `did:plc`. Characters are sovereign
//! data meant to survive both their creator and account deletion. Only the id
//! type exists so far. (DESIGN 5668866)

use std::str::FromStr;

use crate::elements::{did::Did, id::IdError};

/// The identity of a Character: its own [`Did`]. Every actor mints a DID and
/// the DID IS the key, Characters included — there is no separate private id.
/// (DD 57081857)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharacterId(Did);

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
