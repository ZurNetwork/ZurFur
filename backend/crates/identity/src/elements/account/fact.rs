use std::collections::BTreeSet;

use super::errors::UnknownAccountFactKind;

/// A fact that, once an Account reaches it, keeps the Account from a hard delete: its
/// deletion is soft from then on.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = UnknownAccountFactKind,
    parse_err_fn = unknown_token
)]
pub enum AccountFactKind {
    /// The Account has had a member besides its founder.
    Member,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(token: &str) -> UnknownAccountFactKind {
    UnknownAccountFactKind(token.into())
}

/// The facts an Account has reached, each kind once. Empty means its deletion may be
/// hard. Only the member fact is real so far; a commission brought into the Account, an
/// invoice it issued and a public record it published are not yet recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountFacts(BTreeSet<AccountFactKind>);

impl AccountFacts {
    /// Whether the Account has reached no fact.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether the Account has reached `kind`.
    pub fn contains(&self, kind: AccountFactKind) -> bool {
        self.0.contains(&kind)
    }

    /// The kinds reached, in their declared order.
    pub fn iter(&self) -> impl Iterator<Item = AccountFactKind> + '_ {
        self.0.iter().copied()
    }
}

impl FromIterator<AccountFactKind> for AccountFacts {
    fn from_iter<I: IntoIterator<Item = AccountFactKind>>(kinds: I) -> Self {
        Self(kinds.into_iter().collect())
    }
}

impl From<Vec<AccountFactKind>> for AccountFacts {
    fn from(kinds: Vec<AccountFactKind>) -> Self {
        kinds.into_iter().collect()
    }
}

#[cfg(test)]
mod tests;
