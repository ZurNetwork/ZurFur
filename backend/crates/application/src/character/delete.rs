use crate::{
    Ports,
    character::{CharacterError, CharacterResult, Characters},
    common_error::{CommonError, NotFoundEntity},
    ports::WithPorts,
};
use domain::{
    elements::{character::CharacterId, did::DeleteOutcome, user::UserId},
    ports::Unit,
};
use macros::use_case;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub character_id: CharacterId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub outcome: DeleteOutcome,
}

impl Characters<'_> {
    #[use_case]
    pub async fn delete(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
    ) -> CharacterResult<Output> {
        let Command {
            actor_id,
            character_id,
        } = cmd;

        let character =
            ports
                .characters
                .find(&character_id)
                .await?
                .ok_or(CharacterError::Common(CommonError::NotFound(
                    NotFoundEntity::Character,
                )))?;

        if !character.owners.contains(&actor_id) {
            return Err(CharacterError::IncorrectRole);
        }
        let facts = uow.characters().facts_for(&character_id).await?;
        let outcome = character.allowed_deletion_path(&facts);
        match outcome {
            DeleteOutcome::Tombstoned => {
                uow.characters().soft_delete(&character_id).await?;
                // TODO: Tombstone the Character
            }
            DeleteOutcome::Hard => {
                uow.characters().hard_delete(&character_id).await?;
                // TODO: Remove the DID and PDS
            }
        };

        Ok(Output { outcome })
    }
}
