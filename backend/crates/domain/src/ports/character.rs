use async_trait::async_trait;

use crate::elements::character::Character;
use crate::elements::user::UserId;

/// The **read** surface of Zurfur's record of Characters — pool-backed and
/// non-transactional; every write lives on [`CharacterWrites`].
#[async_trait]
pub trait CharacterStore: Send + Sync {
    /// Every live Character `keeper` keeps, ordered by DID. Tombstoned
    /// Characters are left out.
    async fn list_kept_by(&self, keeper: &UserId) -> anyhow::Result<Vec<Character>>;
}

#[async_trait]
pub trait CharacterWrites: Send {
    /// Creates a new character for the specified user with the given attributes.
    async fn create(&self, attributes: Character) -> anyhow::Result<Character>;
}
