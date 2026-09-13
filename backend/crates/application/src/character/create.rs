use domain::{
    datetime::DateTimeUtc,
    elements::{
        character::{Character, CharacterAttributes, CharacterId, Presence},
        handle::{Handle, HandleDomain},
        user::UserId,
    },
};

use crate::{
    account::{AccountError, ensure_handle_claimable},
    character::{CharacterResult, Characters},
    common_error::CommonError,
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub handle: Option<Handle>,
    pub attributes: CharacterAttributes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub character: CharacterId,
}

impl Characters<'_> {
    pub async fn create(
        &self,
        cmd: Command,
        handle_domain: &HandleDomain,
        now: DateTimeUtc,
    ) -> CharacterResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            attributes,
            handle,
        } = cmd;

        let (did, presence) = match handle {
            Some(handle) => {
                ensure_handle_claimable(ports, &handle, handle_domain, None, now)
                    .await
                    .map_err(|e| match e {
                        AccountError::HandleTaken => CommonError::HandleTaken,
                        AccountError::Infrastructure(cause) => CommonError::Infrastructure(cause),
                        other => CommonError::Infrastructure(anyhow::Error::from(other)),
                    })?;
                (
                    ports.did_minter.mint(&handle).await?,
                    Presence::Public { handle },
                )
            }
            None => (ports.did_minter.mint_handleless().await?, Presence::Private),
        };

        let mut uow = ports.database.begin().await?;
        let character = Character::create(actor_id, presence, did, attributes, now);
        let character = uow.characters().create(character).await?;

        uow.commit().await?;
        Ok(Output {
            character: character.id,
        })
    }
}
