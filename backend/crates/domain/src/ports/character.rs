use async_trait::async_trait;

use crate::elements::character::Character;

#[async_trait]
pub trait CharacterStore: Send + Sync {}

#[async_trait]
pub trait CharacterWrites: Send {
    /// Creates a new character for the specified user with the given attributes.
    async fn create(&self, attributes: Character) -> anyhow::Result<Character>;
}
