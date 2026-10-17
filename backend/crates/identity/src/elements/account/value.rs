use crate::string_builder::{StringBuilder, StringBuilderViolation};

/// The longest an account name may be, in `char`s (counted after trimming).
pub const ACCOUNT_NAME_MAX_LEN: usize = 120;

/// A human-readable account name: trimmed, non-empty, at most
/// [`ACCOUNT_NAME_MAX_LEN`] chars.
///
/// ```
/// use domain::elements::account::AccountName;
///
/// let name = "  Acme Studio  ".parse::<AccountName>().unwrap();
/// assert_eq!(name.as_str(), "Acme Studio"); // trimmed
///
/// assert!("   ".parse::<AccountName>().is_err()); // empty after trim
/// assert!("x".repeat(121).parse::<AccountName>().is_err()); // too long
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountName(String);

impl AccountName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The one validating constructor: trim, then check the bounds above.
impl std::str::FromStr for AccountName {
    type Err = super::AccountNameError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(ACCOUNT_NAME_MAX_LEN)
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => super::AccountNameError::Empty,
                StringBuilderViolation::TooLong { len, .. } => super::AccountNameError::TooLong(len),
                StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain never calls no_control.
                    debug_assert!(
                        false,
                        "AccountName's FromStr chain never calls no_control; ControlCharacter is unreachable"
                    );
                    super::AccountNameError::Empty
                }
            })
    }
}

impl AsRef<str> for AccountName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for AccountName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
