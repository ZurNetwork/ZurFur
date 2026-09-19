use crate::{
    Ports,
    character::{CharacterResult, Characters},
    ports::WithPorts,
};
use domain::{
    elements::{character::CharacterId, user::UserId},
    ports::Unit,
};
use macros::use_case;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub character_id: CharacterId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Characters<'_> {
    #[use_case]
    pub async fn depublicize(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
    ) -> CharacterResult<Output> {
        let Command {
            actor_id,
            character_id,
        } = cmd;
        todo!("depublicize character")
    }
}
