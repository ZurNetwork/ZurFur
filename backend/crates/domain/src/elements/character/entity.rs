use crate::{
    datetime::DateTimeUtc,
    elements::{character::CharacterFacts, did::Did, user::UserId},
};

use super::{CharacterAttributes, CharacterId, Presence};
pub use crate::elements::did::DeleteOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    pub presence: Presence,
    pub attributes: CharacterAttributes,
    pub owners: Vec<UserId>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl Character {
    pub fn create(
        // TODO: Make it so that the Vec is not moved in memory as an optimization
        owners: Vec<UserId>,
        presence: Presence,
        did: Did,
        attributes: CharacterAttributes,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: CharacterId(did),
            presence,
            attributes,
            owners,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn allowed_deletion_path(&self, facts: &CharacterFacts) -> DeleteOutcome {
        if facts.active_slots > 0 || facts.gallery_appearances > 0 {
            DeleteOutcome::Tombstoned
        } else {
            DeleteOutcome::Hard
        }
    }
}
