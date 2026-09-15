use serde::{Deserialize, Serialize};

use crate::elements::did::Did;

/// The identity of a [`super::User`]: their [`Did`]. The DID IS the key — there is no
/// separate private surrogate.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
)]
#[as_ref(str)]
pub struct UserId(Did);

impl UserId {
    /// The DID this id is: the actor key itself.
    pub fn did(&self) -> &Did {
        &self.0
    }
}
