//! Identity's arm of the Den for a user's directory: the node at `/user/<did>` and the
//! folders inside it that identity answers for. A user's directory is open only to that user,
//! so every answer here is for the session's viewer alone; another user's DID is absent
//! before any store is read.

use domain::elements::{account::ListingScope, name::NodeName, profile::Profile, user::UserId};

use crate::{
    account::den::AccountPlace,
    character::den::CharacterPlace,
    commission::den::CommissionPlace,
    den::{
        DenContext, DenMount, DenType, Level, Node, NodeState, Place, RootNaming, STANDING,
        Segment, TopLevel,
        names::{self, ACCOUNTS, CHARACTERS, COMMISSIONS, POSTS},
        post::PostPlace,
    },
};

/// The places in a user's directory that this arm answers for. The user's `commissions`
/// folder is the commission arm's (`CommissionPlace::UserCommissions`), and their `posts`
/// entry is a mount of `/post/<did>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UserPlace {
    /// The user's own directory, `/user/<did>`.
    User(UserId),
    /// The user's `accounts` folder.
    Accounts(UserId),
    /// The user's `characters` folder.
    Characters(UserId),
}

impl UserPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            UserPlace::User(_) => DenType::User,
            UserPlace::Accounts(_) => DenType::UserAccounts,
            UserPlace::Characters(_) => DenType::UserCharacters,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            UserPlace::User(user) => names::did_segment(user.did()),
            UserPlace::Accounts(_) => names::fixed_segment(ACCOUNTS),
            UserPlace::Characters(_) => names::fixed_segment(CHARACTERS),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            UserPlace::User(_) => Some(TopLevel::User),
            UserPlace::Accounts(_) | UserPlace::Characters(_) => None,
        }
    }
}

/// The user at `/user/<user>`: the viewer's own root, Private and lifted by their standing,
/// named from their own profile. Absent for any other user, with no store or profile call,
/// and when the viewer has no user row.
pub(crate) async fn node(context: &DenContext<'_>, user: &UserId) -> anyhow::Result<Option<Node>> {
    if user != context.viewer {
        return Ok(None);
    }
    if context.ports.users.find(context.viewer).await?.is_none() {
        return Ok(None);
    }
    let Some(name) = root_name(context).await else {
        return Ok(None);
    };
    let node = Node {
        place: Place::User(UserPlace::User(context.viewer.clone())),
        name,
        level: Level::Private,
        lifts: vec![STANDING],
        state: NodeState::Live,
    };
    Ok(Some(node))
}

/// The viewer's root name: the first name of their own profile the name rule accepts, else
/// their DID. The profile comes from the cache, else, unless the read names from the cache
/// only, from one lookup of the viewer's own DID that only the first read waits on; a miss,
/// a failure, a busy lookup or a profile for another DID names the root by its DID.
async fn root_name(context: &DenContext<'_>) -> Option<NodeName> {
    let profile = viewer_profile(context).await;
    let own_profile = profile.filter(|profile| &profile.did == context.viewer.did());
    let from_profile = own_profile.as_ref().and_then(|profile| {
        profile
            .name_candidates()
            .find_map(|candidate| NodeName::try_from(candidate).ok())
    });
    from_profile.or_else(|| NodeName::try_from(context.viewer.as_ref()).ok())
}

/// The viewer's profile as the read's root naming allows: the cache, then a lookup only when
/// the read may look up.
async fn viewer_profile(context: &DenContext<'_>) -> Option<Profile> {
    let ports = context.ports;
    let did = context.viewer.did();
    match context.root_naming {
        RootNaming::LookUp => {
            ports
                .profile_lookups
                .profile_unless_busy(&ports.profile_cache, &ports.profile_source, did)
                .await
        }
        RootNaming::CacheOnly => ports.profile_cache.get(did).await.ok().flatten(),
    }
}

/// The node at `key` in the folder `place`: one of the viewer's system folders, matched by
/// its fixed segment, with no store call. The `accounts` and `characters` folders hold no
/// real children.
pub(crate) async fn child(
    context: &DenContext<'_>,
    place: &UserPlace,
    key: &Segment,
) -> anyhow::Result<Option<Node>> {
    let folders = children(context, place).await?;
    let folder = folders
        .into_iter()
        .find(|folder| folder.place.key().as_ref() == Some(key));
    Ok(folder)
}

/// Every real child of the folder `place`: the viewer's own root holds the system folders
/// `accounts`, `characters` and `commissions`; nothing else holds real children.
pub(crate) async fn children(
    context: &DenContext<'_>,
    place: &UserPlace,
) -> anyhow::Result<Vec<Node>> {
    match place {
        UserPlace::User(user) if user == context.viewer => Ok(system_folders(user)),
        UserPlace::User(_) | UserPlace::Accounts(_) | UserPlace::Characters(_) => Ok(Vec::new()),
    }
}

/// The viewer's mounts in the folder `place`, each read with the session's own id: their
/// `posts` in their root, an Account per membership and per deactivated Account they own in
/// `accounts`, and a Character per one they keep in `characters`. Any other user's folder
/// holds none.
pub(crate) async fn mounts(
    context: &DenContext<'_>,
    place: &UserPlace,
) -> anyhow::Result<Vec<DenMount>> {
    match place {
        UserPlace::User(user) if user == context.viewer => Ok(posts_mount(context)),
        UserPlace::Accounts(user) if user == context.viewer => account_mounts(context).await,
        UserPlace::Characters(user) if user == context.viewer => character_mounts(context).await,
        UserPlace::User(_) | UserPlace::Accounts(_) | UserPlace::Characters(_) => Ok(Vec::new()),
    }
}

/// The system folders in `user`'s root: Private, with no lifts of their own, since the
/// root's standing covers its real subtree.
fn system_folders(user: &UserId) -> Vec<Node> {
    let folders = [
        (Place::User(UserPlace::Accounts(user.clone())), ACCOUNTS),
        (Place::User(UserPlace::Characters(user.clone())), CHARACTERS),
        (
            Place::Commission(CommissionPlace::UserCommissions(user.clone())),
            COMMISSIONS,
        ),
    ];
    folders
        .into_iter()
        .filter_map(|(place, word)| system_folder(place, word))
        .collect()
}

/// One system folder at `place`, named by its fixed `word`: Private, with no lifts of its
/// own, and live.
pub(crate) fn system_folder(place: Place, word: &'static str) -> Option<Node> {
    let name = names::fixed_name(word)?;
    let folder = Node {
        place,
        name,
        level: Level::Private,
        lifts: Vec::new(),
        state: NodeState::Live,
    };
    Some(folder)
}

/// A mount of `target` at its own key.
fn mount_at_key(target: Place) -> Option<DenMount> {
    let segment = target.key()?;
    let mount = DenMount { segment, target };
    Some(mount)
}

/// The viewer's `posts` mount of `/post/<their DID>`, den's empty posts stub.
fn posts_mount(context: &DenContext<'_>) -> Vec<DenMount> {
    let Some(segment) = names::fixed_segment(POSTS) else {
        return Vec::new();
    };
    let target = Place::Post(PostPlace::Posts(context.viewer.did().clone()));
    let posts = DenMount { segment, target };
    vec![posts]
}

/// A mount of each Account the viewer is a member of, and of each deactivated Account they
/// own, at the Account's DID; den's admission hides the deactivated ones unless asked.
async fn account_mounts(context: &DenContext<'_>) -> anyhow::Result<Vec<DenMount>> {
    let accounts = &context.ports.accounts;
    let memberships = accounts
        .list_for_user(context.viewer, ListingScope::SelfView)
        .await?;
    let deactivated = accounts.list_deactivated_for_owner(context.viewer).await?;
    let live_ids = memberships
        .into_iter()
        .map(|membership| membership.account.id);
    let deactivated_ids = deactivated.into_iter().map(|account| account.id);
    let mounts = live_ids
        .chain(deactivated_ids)
        .filter_map(|account| mount_at_key(Place::Account(AccountPlace::Account(account))))
        .collect();
    Ok(mounts)
}

/// A mount of each Character the viewer keeps, at the Character's DID.
async fn character_mounts(context: &DenContext<'_>) -> anyhow::Result<Vec<DenMount>> {
    let kept = context
        .ports
        .characters
        .list_kept_by(context.viewer)
        .await?;
    let mounts = kept
        .into_iter()
        .filter_map(|character| {
            mount_at_key(Place::Character(CharacterPlace::Character(character.id)))
        })
        .collect();
    Ok(mounts)
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
