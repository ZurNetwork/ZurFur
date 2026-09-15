use crate::elements::did::Did;

/// The identity of a Character: its own [`Did`]. Every actor mints a DID and
/// the DID IS the key, Characters included — there is no separate private id.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::AsRef,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct CharacterId(pub(super) Did);

impl CharacterId {
    /// The DID this id is: the actor key itself.
    pub fn did(&self) -> &Did {
        &self.0
    }
}
