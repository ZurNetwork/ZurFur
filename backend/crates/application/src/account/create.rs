use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountName, AccountNameError},
        handle::{Handle, HandleDomain},
        user::UserId,
    },
};

use crate::{
    Ports,
    account::{AccountError, AccountResult, Accounts, ServedName, ensure_handle_claimable},
    den::{DenNameError, ShowableName, showable_refusal},
};

pub struct Command {
    pub actor_id: UserId,
    pub name: AccountName,
    pub handle: Handle,
}
pub struct Output {
    pub account_id: AccountId,
    pub handle: Handle,
    pub name: ServedName,
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
        let name = showable_account_name(&name)?;
        ensure_handle_claimable(ports, &handle, handle_domain, None, now).await?;

        let did = ports.did_minter.mint(&handle).await?;

        let (account, owner) = Account::open(actor_id, did, handle, name, now);
        let uow = uow.open().await?;
        uow.accounts().create(&account, &owner).await?;
        let name = ServedName::from(&account);
        Ok(Output {
            account_id: account.id,
            handle: account.handle,
            name,
        })
    }
}

/// An Account's name as the rule for a name safe to show keeps it: refused where that rule
/// refuses, a `/` allowed, and stored without the characters the rule strips.
fn showable_account_name(name: &AccountName) -> AccountResult<AccountName> {
    let kept = ShowableName::try_from(name.as_str())
        .map_err(|error| AccountError::InvalidName(showable_refusal(error)))?;
    kept.as_ref().parse().map_err(|error| {
        let refusal = match error {
            AccountNameError::Empty => DenNameError::Empty,
            AccountNameError::TooLong(_) => DenNameError::TooLong,
        };
        AccountError::InvalidName(refusal)
    })
}
