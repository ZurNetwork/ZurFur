//! [`CharacterId`] — the identity of a Character. **Stub.**
//!
//! A Character is a repository and representation of a character, kept by one or
//! more Keepers and identified by its own `did:plc`. Characters are sovereign
//! data meant to survive both their creator and account deletion. Only the id
//! type exists so far.

use std::{ops::Deref, str::FromStr};

use crate::{
    datetime::DateTimeUtc,
    elements::{did::Did, handle::Handle, id::IdError, user::UserId},
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The identity of a Character: its own [`Did`]. Every actor mints a DID and
/// the DID IS the key, Characters included — there is no separate private id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::AsRef)]
#[as_ref(str)]
pub struct CharacterId(Did);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterName(String);

impl FromStr for CharacterName {
    type Err = StringBuilderViolation;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StringBuilder::non_empty_from(s).build().map(CharacterName)
    }
}

impl Deref for CharacterName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for CharacterName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterDescription(Option<String>);

impl FromStr for CharacterDescription {
    type Err = StringBuilderViolation;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(CharacterDescription(None))
        } else {
            StringBuilder::non_empty_from(s)
                .build()
                .map(|desc| CharacterDescription(Some(desc)))
        }
    }
}

impl Deref for CharacterDescription {
    type Target = Option<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    pub presence: Presence,
    pub attributes: CharacterAttributes,
    pub owner_id: UserId,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    Private,
    Public { handle: Handle },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicCharacterAttribute {
    pub name: String,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterAttributes {
    pub name: CharacterName,
    pub description: CharacterDescription,
    pub species: String,
    pub dynamic_attributes: Vec<DynamicCharacterAttribute>,
}

impl Character {
    pub fn create(
        owner_id: UserId,
        presence: Presence,
        did: Did,
        attributes: CharacterAttributes,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: CharacterId(did),
            presence,
            attributes,
            owner_id,
            created_at: now,
            updated_at: now,
        }
    }
}
