//! In-memory fake for the Character write port.

use async_trait::async_trait;
use domain::elements::character::Character;
use domain::ports::character::{CharacterStore, CharacterWrites};

use crate::MemBackend;

/// Reads arrive with the Character slice; the port declares none yet.
pub struct MemCharacterStore;

#[async_trait]
impl CharacterStore for MemCharacterStore {}

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
        characters.insert(attributes.id.clone(), attributes.clone());
        Ok(attributes)
    }
}
