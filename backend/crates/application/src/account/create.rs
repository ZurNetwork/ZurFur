use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountName},
        handle::{Handle, HandleDomain},
        user::UserId,
    },
};

use crate::{
    Ports,
    account::{AccountResult, Accounts, ensure_handle_claimable},
};

pub struct Command {
    pub actor_id: UserId,
    pub name: AccountName,
    pub handle: Handle,
}
pub struct Output {
    pub account_id: AccountId,
    pub handle: Handle,
    pub name: AccountName,
}

impl Accounts<'_> {
    #[use_case]
    pub async fn create(
        &self,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        #[ports] ports: &Ports,
        cmd: Command,
        handle_domain: &HandleDomain,
        now: DateTimeUtc,
    ) -> AccountResult<Output> {
        let Command {
            actor_id,
            handle,
            name,
        } = cmd;
        ensure_handle_claimable(ports, &handle, handle_domain, None, now).await?;

        let did = ports.did_minter.mint(&handle).await?;

        let (account, owner) = Account::open(actor_id, did, handle, name, now);
        let uow = uow.open().await?;
        uow.accounts().create(&account, &owner).await?;
        Ok(Output {
            account_id: account.id,
            handle: account.handle,
            name: account.name,
        })
    }
}
