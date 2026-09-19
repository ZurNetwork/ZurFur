use async_trait::async_trait;

use crate::elements::character::{Character, CharacterFacts, CharacterId};

#[async_trait]
pub trait CharacterStore: Send + Sync {
    async fn find(&self, character_id: &CharacterId) -> anyhow::Result<Option<Character>>;
}

#[async_trait]
pub trait CharacterWrites: Send {
    /// Exists in the transaction context and retrieves the current facts for the character.
    async fn facts_for(&self, character_id: &CharacterId) -> anyhow::Result<CharacterFacts>;
    /// Creates a new character for the specified user with the given attributes.
    async fn create(&self, attributes: Character) -> anyhow::Result<Character>;
    async fn soft_delete(&self, character_id: &CharacterId) -> anyhow::Result<()>;
    async fn hard_delete(&self, character_id: &CharacterId) -> anyhow::Result<()>;
}
