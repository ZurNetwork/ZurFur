//! [`CharacterId`] — the identity of a Character. **Stub.**
//!
//! A Character is a repository and representation of a character, kept by one or
//! more Keepers and identified by its own `did:plc`. Characters are sovereign
//! data meant to survive both their creator and account deletion. Only the id
//! type exists so far.

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
