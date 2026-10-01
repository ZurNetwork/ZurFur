use serde::Deserialize;

use crate::elements::did::Did;

/// An [`Account`]'s identifier: its [`Did`] — the DID is the only identifier,
/// with no separate surrogate id behind it.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
)]
#[serde(transparent)]
#[as_ref(str)]
pub struct AccountId(Did);

impl AccountId {
    /// The DID this id is: the actor key itself.
    pub fn did(&self) -> &Did {
        &self.0
    }
}
