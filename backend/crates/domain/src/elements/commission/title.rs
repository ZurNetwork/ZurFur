use super::CommissionTitleError;
use crate::string_builder::{StringBuilder, StringBuilderViolation};

/// A commission's Title: trimmed, and non-empty. No length cap yet.
///
/// ```
/// use domain::elements::commission::CommissionTitle;
///
/// let title = "  A ref sheet  ".parse::<CommissionTitle>().unwrap();
/// assert_eq!(title.as_str(), "A ref sheet"); // trimmed
///
/// assert!("   ".parse::<CommissionTitle>().is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionTitle(String);

impl CommissionTitle {
    /// The validated, trimmed title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommissionTitle {
    type Error = CommissionTitleError;

    /// Validate and wrap a title: trim, then reject an empty result.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => CommissionTitleError::Empty,
                StringBuilderViolation::TooLong { .. }
                | StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain only applies trimmed().non_empty().
                    debug_assert!(
                        false,
                        "CommissionTitle's TryFrom chain only applies trimmed().non_empty()"
                    );
                    CommissionTitleError::Empty
                }
            })
    }
}

impl std::str::FromStr for CommissionTitle {
    type Err = CommissionTitleError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for CommissionTitle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
