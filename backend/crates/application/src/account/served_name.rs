use domain::elements::account::Account;

use crate::den::ShowableName;

/// An Account's name as every Account output serves it: the stored name as the rule for a
/// name safe to show keeps it, else the Account's handle. The stored name never leaves as-is.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct ServedName(String);

impl From<&Account> for ServedName {
    /// The kept form of `account`'s stored name when the rule accepts it, else its handle.
    fn from(account: &Account) -> Self {
        let served = ShowableName::try_from(account.name.as_str())
            .map(|kept| kept.to_string())
            .unwrap_or_else(|_| account.handle.as_str().to_owned());
        Self(served)
    }
}

#[cfg(test)]
mod tests;
