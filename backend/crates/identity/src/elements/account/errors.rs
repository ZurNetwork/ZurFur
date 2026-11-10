use super::ACCOUNT_NAME_MAX_LEN;

/// Why a string was rejected as an account name.
///
/// ```
/// use identity::elements::account::{AccountName, AccountNameError};
///
/// assert_eq!("".parse::<AccountName>(), Err(AccountNameError::Empty));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccountNameError {
    /// Empty once trimmed.
    #[error("account name must not be empty")]
    Empty,
    /// Longer than [`ACCOUNT_NAME_MAX_LEN`] chars; carries the length.
    #[error("account name is {0} chars; the max is {ACCOUNT_NAME_MAX_LEN}")]
    TooLong(usize),
}

/// A stored fact kind outside the known ones: a schema-drift signal, not user input;
/// carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown account fact kind {0:?}")]
pub struct UnknownAccountFactKind(pub String);
