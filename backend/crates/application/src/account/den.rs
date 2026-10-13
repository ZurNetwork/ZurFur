//! Identity's arm of the Den for an Account's directory: the node at `/account/<did>` and
//! the folders inside it that identity answers for.

use domain::elements::account::{Account, AccountId};

use crate::{
    account::workflow::den::WorkflowPlace,
    commission::den::CommissionPlace,
    den::{
        DenContext, DenMount, DenType, Level, Node, NodeName, NodeState, Place, STANDING, Segment,
        TopLevel,
        names::{self, CHARACTERS, COMMISSIONS, WORKFLOWS},
    },
    user::den::system_folder,
};

/// The places in an Account's directory that this arm answers for. The Account's
/// `commissions` folder is the commission arm's (`CommissionPlace::AccountCommissions`), and
/// its `workflows` folder is the Workflow arm's (`WorkflowPlace::Workflows`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccountPlace {
    /// The Account's directory, `/account/<did>`.
    Account(AccountId),
    /// The Account's `characters` folder.
    Characters(AccountId),
}

impl AccountPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            AccountPlace::Account(_) => DenType::Account,
            AccountPlace::Characters(_) => DenType::AccountCharacters,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            AccountPlace::Account(account) => names::did_segment(account.did()),
            AccountPlace::Characters(_) => names::fixed_segment(CHARACTERS),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            AccountPlace::Account(_) => Some(TopLevel::Account),
            AccountPlace::Characters(_) => None,
        }
    }
}

/// The Account at `/account/<account>`, for the session's viewer: open by their standing when
/// it is live and they hold a role in it, or when it is deactivated and they are its Owner;
/// absent otherwise, a live Account they hold no role in included. It reads the same three
/// stores in the same order whatever the Account's state, so an absent Account and another's
/// can't be told apart by what runs.
pub(crate) async fn node(
    context: &DenContext<'_>,
    account: &AccountId,
) -> anyhow::Result<Option<Node>> {
    let accounts = &context.ports.accounts;
    let live = accounts.find(account).await?;
    let role = accounts.role_of(context.viewer, account).await?;
    let deactivated = accounts.list_deactivated_for_owner(context.viewer).await?;
    let opened = match (live, role) {
        (Some(live), Some(_)) => Some((live, NodeState::Live)),
        (Some(_), None) => None,
        (None, _) => deactivated
            .into_iter()
            .find(|owned| &owned.id == account)
            .map(|owned| {
                let state = NodeState::Deactivated {
                    viewer_is_owner: true,
                };
                (owned, state)
            }),
    };
    let node = opened.and_then(|(opened, state)| account_node(&opened, state));
    Ok(node)
}

/// The node for an Account the viewer may open: Private, lifted by their standing.
fn account_node(account: &Account, state: NodeState) -> Option<Node> {
    let name = account_name(account)?;
    let node = Node {
        place: Place::Account(AccountPlace::Account(account.id.clone())),
        name,
        level: Level::Private,
        lifts: vec![STANDING],
        state,
    };
    Some(node)
}

/// An Account's name: its stored name when the name rule accepts it, else its handle, never
/// `Unnamed`. A handle always passes the rule, which a test pins.
fn account_name(account: &Account) -> Option<NodeName> {
    NodeName::try_from(account.name.as_ref())
        .or_else(|_| NodeName::try_from(account.handle.as_ref()))
        .ok()
}

/// The node at `key` in the folder `place`: one of the Account's system folders, matched by
/// its fixed segment, with no store call.
pub(crate) async fn child(
    context: &DenContext<'_>,
    place: &AccountPlace,
    key: &Segment,
) -> anyhow::Result<Option<Node>> {
    let folders = children(context, place).await?;
    let folder = folders
        .into_iter()
        .find(|folder| folder.place.key().as_ref() == Some(key));
    Ok(folder)
}

/// Every real child of the folder `place`. An Account holds the system folders
/// `characters`, `commissions` and `workflows`, reached only once the host answered for the
/// Account itself. Its `characters` folder is empty: the shared Characters an Account
/// administers aren't modeled yet, and the slice that models them replaces this answer.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    place: &AccountPlace,
) -> anyhow::Result<Vec<Node>> {
    match place {
        AccountPlace::Account(account) => Ok(system_folders(account)),
        AccountPlace::Characters(_) => Ok(Vec::new()),
    }
}

/// The viewer's mounts in the folder `place`: none, since no shared Character is modeled yet.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &AccountPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}

/// The system folders in `account`'s directory: Private, with no lifts of their own, since
/// the Account's standing covers its real subtree.
fn system_folders(account: &AccountId) -> Vec<Node> {
    let folders = [
        (
            Place::Account(AccountPlace::Characters(account.clone())),
            CHARACTERS,
        ),
        (
            Place::Commission(CommissionPlace::AccountCommissions(account.clone())),
            COMMISSIONS,
        ),
        (
            Place::Workflow(WorkflowPlace::Workflows(account.clone())),
            WORKFLOWS,
        ),
    ];
    folders
        .into_iter()
        .filter_map(|(place, word)| system_folder(place, word))
        .collect()
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
