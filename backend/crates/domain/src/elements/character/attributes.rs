use std::str::FromStr;

use crate::elements::text::{NonEmptyString, NonEmptyStringError};

/// A Character's name: trimmed, never blank.
#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef, derive_more::FromStr,
)]
#[as_ref(str)]
pub struct CharacterName(NonEmptyString);

/// A Character's free-text description. Empty input means no description;
/// whitespace-only input is refused like any other blank text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterDescription(Option<NonEmptyString>);

impl CharacterDescription {
    /// The description text, or `None` when there is none.
    pub fn as_deref(&self) -> Option<&str> {
        self.0.as_ref().map(|text| text.as_ref())
    }
}

impl FromStr for CharacterDescription {
    type Err = NonEmptyStringError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self(None));
        }
        s.parse::<NonEmptyString>().map(|text| Self(Some(text)))
    }
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
