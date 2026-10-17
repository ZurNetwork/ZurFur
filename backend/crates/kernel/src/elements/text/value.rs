use std::str::FromStr;

use crate::string_builder::{StringBuilder, StringBuilderViolation};

use super::NonEmptyStringError;

/// A trimmed, never-blank string: the one rule every free-text name shares.
/// Wrappers such as `CharacterName` forward their parsing here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct NonEmptyString(String);

impl TryFrom<String> for NonEmptyString {
    type Error = NonEmptyStringError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::non_empty_from(raw)
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => NonEmptyStringError::Empty,
                StringBuilderViolation::TooLong { .. }
                | StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain calls neither max_chars nor no_control.
                    debug_assert!(false, "NonEmptyString's chain only trims and refuses empty");
                    NonEmptyStringError::Empty
                }
            })
    }
}

impl FromStr for NonEmptyString {
    type Err = NonEmptyStringError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

#[cfg(test)]
mod tests;
