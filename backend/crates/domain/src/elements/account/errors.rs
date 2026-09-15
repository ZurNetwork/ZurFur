use super::ACCOUNT_NAME_MAX_LEN;

/// Why a string was rejected as an account name.
///
/// ```
/// use domain::elements::account::{AccountName, AccountNameError};
///
/// assert_eq!("".parse::<AccountName>(), Err(AccountNameError::Empty));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountNameError {
    /// Empty once trimmed.
    Empty,
    /// Longer than [`ACCOUNT_NAME_MAX_LEN`] chars; carries the length.
    TooLong(usize),
}

impl std::fmt::Display for AccountNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountNameError::Empty => write!(f, "account name must not be empty"),
            AccountNameError::TooLong(len) => write!(
                f,
                "account name is {len} chars; the max is {ACCOUNT_NAME_MAX_LEN}"
            ),
        }
    }
}

impl std::error::Error for AccountNameError {}
