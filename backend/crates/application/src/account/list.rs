use domain::elements::{
    account::{AccountId, AccountName, ListingScope},
    handle::Handle,
    role::{Role, RoleAlias},
    user::UserId,
};

use crate::account::{AccountResult, Accounts};

pub struct Query {
    pub user_id: UserId,
}

pub struct Listing {
    pub id: AccountId,
    pub handle: Handle,
    pub name: AccountName,
    pub role: Role,
    pub alias: Option<RoleAlias>,
}
pub struct Output {
    pub accounts: Vec<Listing>,
}

impl<'a> Accounts<'a> {
    pub async fn list(&self, query: Query) -> AccountResult<Output> {
        let ports = self.ports();

        Ok(Output {
            accounts: ports
                .accounts
                .list_for_user(&query.user_id, ListingScope::SelfView)
                .await?
                .into_iter()
                .map(|m| Listing {
                    id: m.account.id,
                    handle: m.account.handle,
                    name: m.account.name,
                    role: m.role,
                    alias: m.alias,
                })
                .collect(),
        })
    }
}
