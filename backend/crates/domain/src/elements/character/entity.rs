use crate::{
    datetime::DateTimeUtc,
    elements::{did::Did, user::UserId},
};

use super::{CharacterAttributes, CharacterId, Presence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    pub presence: Presence,
    pub attributes: CharacterAttributes,
    pub owner_id: UserId,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl Character {
    pub fn create(
        owner_id: UserId,
        presence: Presence,
        did: Did,
        attributes: CharacterAttributes,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: CharacterId(did),
            presence,
            attributes,
            owner_id,
            created_at: now,
            updated_at: now,
        }
    }
}
