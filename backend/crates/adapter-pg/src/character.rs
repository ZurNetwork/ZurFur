//! PostgreSQL adapter for [`domain::ports::character::CharacterWrites`].
//! Stub — no storage lands until the Character slice.

use async_trait::async_trait;
use domain::elements::character::Character;
use domain::ports::character::{CharacterStore, CharacterWrites};

/// Reads arrive with the Character slice; the port declares none yet.
pub struct PgCharacterStore;

#[async_trait]
impl CharacterStore for PgCharacterStore {}

/// The character write view over one open transaction. Stubbed: holds no
/// connection because there is nothing to persist to yet.
pub struct PgCharacterWrites;

#[async_trait]
impl CharacterWrites for PgCharacterWrites {
    async fn create(&self, _attributes: Character) -> anyhow::Result<Character> {
        unimplemented!("character persistence lands with the Character slice")
    }
}
