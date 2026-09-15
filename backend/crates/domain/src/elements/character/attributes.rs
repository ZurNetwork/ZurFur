use std::{ops::Deref, str::FromStr};

use crate::string_builder::{StringBuilder, StringBuilderViolation};

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
