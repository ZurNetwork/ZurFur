use crate::use_case;
use domain::elements::{
    account::{AccountId, AccountMembership, AccountName, ListingScope},
    handle::Handle,
    role::{Role, RoleAlias},
    user::UserId,
};

use crate::{
    Ports,
    account::{AccountError, AccountResult, Accounts},
    den::{
        DenError,
        access::{Opens, ServedNode, may_open},
    },
};

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
    /// The viewer's live memberships, in the store's order, keeping only the Accounts den's
    /// access check opens for them.
    #[use_case]
    pub async fn list(&self, #[ports] ports: &Ports, query: Query) -> AccountResult<Output> {
        let memberships = ports
            .accounts
            .list_for_user(&query.user_id, ListingScope::SelfView)
            .await?;
        let mut accounts = Vec::with_capacity(memberships.len());
        for membership in memberships {
            let Some(opens) = opened_account(ports, &query.user_id, &membership).await? else {
                continue;
            };
            accounts.push(listing(&opens, membership)?);
        }
        Ok(Output { accounts })
    }
}

/// The check's proof that `viewer` opens `membership`'s Account; `None` when it doesn't.
async fn opened_account(
    ports: &Ports,
    viewer: &UserId,
    membership: &AccountMembership,
) -> AccountResult<Option<Opens>> {
    let served = ServedNode::Account(membership.account.id.clone());
    match may_open(ports, viewer, served).await {
        Ok(opens) => Ok(Some(opens)),
        Err(DenError::NotFound) => Ok(None),
        Err(DenError::Store(error)) => Err(AccountError::Infrastructure(error)),
    }
}

/// The listing for `membership`, named by the Account id the check opened; a proof for any
/// other node fails the read.
fn listing(opens: &Opens, membership: AccountMembership) -> AccountResult<Listing> {
    let ServedNode::Account(id) = opens.node() else {
        return Err(proof_mismatch());
    };
    if id != &membership.account.id {
        return Err(proof_mismatch());
    }
    let listing = Listing {
        id: id.clone(),
        handle: membership.account.handle,
        name: membership.account.name,
        role: membership.role,
        alias: membership.alias,
    };
    Ok(listing)
}

/// The failure for a check whose proof names a node other than the membership's Account.
fn proof_mismatch() -> AccountError {
    let mismatch = anyhow::anyhow!("den's check opened a node it wasn't asked about");
    AccountError::Infrastructure(mismatch)
}
