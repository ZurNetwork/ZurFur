//! In-memory fakes for the Character ports.

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::character::Character;
use domain::elements::user::UserId;
use domain::ports::character::{CharacterStore, CharacterWrites};

use crate::MemBackend;

/// A Character as the mem backend keeps it: the entity plus its tombstone,
/// which no write path sets yet. `PartialEq` lets the unit merge tell an
/// untouched row from a written one.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredCharacter {
    /// The Character itself.
    pub(crate) character: Character,
    /// When the Character was tombstoned; `Some` hides it from every read.
    pub(crate) tombstoned_at: Option<DateTimeUtc>,
}

/// In-memory [`CharacterStore`] read surface over the shared [`MemBackend`].
pub struct MemCharacterStore(pub(crate) MemBackend);

#[async_trait]
impl CharacterStore for MemCharacterStore {
    /// Scan for `keeper`'s live Characters, sorted by DID by byte value.
    async fn list_kept_by(&self, keeper: &UserId) -> anyhow::Result<Vec<Character>> {
        let characters = self
            .0
            .characters
            .lock()
            .expect("MemBackend characters mutex poisoned");
        let mut kept: Vec<Character> = characters
            .values()
            .filter(|stored| stored.tombstoned_at.is_none())
            .filter(|stored| &stored.character.owner_id == keeper)
            .map(|stored| stored.character.clone())
            .collect();
        kept.sort_by(|left, right| {
            AsRef::<str>::as_ref(&left.id).cmp(AsRef::<str>::as_ref(&right.id))
        });
        Ok(kept)
    }
}

/// In-memory [`CharacterWrites`] view, vended by
/// [`MemUnitOfWork::characters`](crate::MemUnitOfWork) over the unit's staging
/// snapshot, so a write reaches the shared store only on commit.
pub struct MemCharacterWrites(pub(crate) MemBackend);

#[async_trait]
impl CharacterWrites for MemCharacterWrites {
    async fn create(&self, attributes: Character) -> anyhow::Result<Character> {
        let mut characters = self
            .0
            .characters
            .lock()
            .expect("MemBackend characters mutex poisoned");
        // Mirror the pg PK: creating the same id twice is a caller bug.
        anyhow::ensure!(
            !characters.contains_key(&attributes.id),
            "character already exists: {}",
            AsRef::<str>::as_ref(&attributes.id)
        );
        let stored = StoredCharacter {
            character: attributes.clone(),
            tombstoned_at: None,
        };
        characters.insert(attributes.id.clone(), stored);
        Ok(attributes)
    }
}

#[cfg(test)]
mod tests;
