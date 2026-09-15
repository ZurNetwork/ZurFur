use std::str::FromStr;

use crate::string_builder::{StringBuilder, StringBuilderViolation};

/// A Slot's title — the one required facet of a declared Slot: trimmed, and
/// non-empty. No length cap yet.
///
/// ```
/// use domain::elements::commission::SlotTitle;
///
/// let title = "  The knight  ".parse::<SlotTitle>().unwrap();
/// assert_eq!(title.as_str(), "The knight"); // trimmed
///
/// assert!("   ".parse::<SlotTitle>().is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotTitle(String);

/// Parses a Slot title: trimmed, then non-empty or [`SlotTitleError::Empty`].
impl FromStr for SlotTitle {
    type Err = super::SlotTitleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StringBuilder::new(s)
            .trimmed()
            .non_empty()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => super::SlotTitleError::Empty,
                StringBuilderViolation::TooLong { .. }
                | StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain only applies trimmed().non_empty().
                    debug_assert!(
                        false,
                        "SlotTitle's FromStr chain only applies trimmed().non_empty()"
                    );
                    super::SlotTitleError::Empty
                }
            })
    }
}

impl AsRef<str> for SlotTitle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl SlotTitle {
    /// The validated, trimmed title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
