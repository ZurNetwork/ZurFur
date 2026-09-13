use domain::elements::{character::CharacterId, user::UserId};

use crate::character::{CharacterResult, Characters};

// QUICK NOTE: A character, like commissions, cannot be deleted when they contain hard facts. They may only be soft-deleted.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub character_id: CharacterId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Characters<'_> {
    pub async fn delete(&self, cmd: Command) -> CharacterResult<Output> {
        let Command {
            actor_id: _,
            character_id: _,
        } = cmd;
        todo!("Characters::delete lands with the Character slice")
    }
}
