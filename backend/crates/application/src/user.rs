//! Use cases about the acting [`User`].

use domain::elements::{
    profile::Profile,
    user::{User, UserId},
};
use domain::ports::{ProfileCache, ProfileSource, UserStore};

/// Who the caller is: the recognized [`User`] and their public profile when
/// it could be resolved. `profile` is `None` when neither the cache nor the
/// PDS answered — absence is not an error (R4), the drivers render the bare
/// DID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    pub user: User,
    pub profile: Option<Profile>,
}

/// Why [`me`] could not answer.
#[derive(Debug)]
pub enum MeError {
    /// No user carries `id` — a stale session or identity file.
    UnknownUser(UserId),
    /// The user store failed.
    Store(anyhow::Error),
}

impl std::fmt::Display for MeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeError::UnknownUser(id) => write!(f, "no user with id {id:?}"),
            MeError::Store(e) => write!(f, "user store failed: {e:#}"),
        }
    }
}

impl std::error::Error for MeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MeError::UnknownUser(_) => None,
            MeError::Store(e) => Some(e.as_ref()),
        }
    }
}

/// `GET /me` and `zurfur session whoami`: load the user behind `id` and
/// resolve their profile read-through ([`Profile::resolve_through`]). How
/// `id` was established — a session, an identity file — is the driver's.
pub async fn me(
    users: &dyn UserStore,
    profile_cache: &dyn ProfileCache,
    profile_source: &dyn ProfileSource,
    id: UserId,
) -> Result<Me, MeError> {
    let user = users
        .find(id)
        .await
        .map_err(MeError::Store)?
        .ok_or(MeError::UnknownUser(id))?;
    let profile = Profile::resolve_through(profile_cache, profile_source, &user.did).await;
    Ok(Me { user, profile })
}
