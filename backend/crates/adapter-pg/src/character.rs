//! PostgreSQL adapter for the Character ports. No Character can be stored in
//! PostgreSQL yet: the keeper read answers empty and `create` is unimplemented
//! until character storage lands.

use async_trait::async_trait;
use domain::elements::character::Character;
use domain::elements::user::UserId;
use domain::ports::character::{CharacterStore, CharacterWrites};

/// The Character read port over PostgreSQL. Holds no pool: there is no
/// character table to read yet.
pub struct PgCharacterStore;

#[async_trait]
impl CharacterStore for PgCharacterStore {
    /// Always empty: PostgreSQL stores no Character yet, so none is kept. This
    /// answer turns false when character storage lands, which replaces it.
    async fn list_kept_by(&self, _keeper: &UserId) -> anyhow::Result<Vec<Character>> {
        Ok(Vec::new())
    }
}

/// The character write view over one open transaction. Stubbed: holds no
/// connection because there is nothing to persist to yet.
pub struct PgCharacterWrites;

#[async_trait]
impl CharacterWrites for PgCharacterWrites {
    async fn create(&self, _attributes: Character) -> anyhow::Result<Character> {
        unimplemented!("character persistence lands with the Character slice")
    }
}

#[cfg(test)]
mod tests;
