//! [`Character`] — a sona or OC, identified by its own `did:plc` and kept by
//! one User, its Keeper. Characters are sovereign data meant to survive both
//! their creator and account deletion. Zurfur's own record names the Keeper;
//! no public ownership claim is modelled yet.

mod attributes;
mod entity;
mod id;
mod presence;

pub use attributes::{
    CharacterAttributes, CharacterDescription, CharacterName, DynamicCharacterAttribute,
};
pub use entity::Character;
pub use id::CharacterId;
pub use presence::Presence;
