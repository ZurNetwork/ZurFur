use domain::{
    elements::{profile::Profile, user::UserId},
    ports::{ProfileCache, ProfileSource},
};

use crate::user::Users;

/// `me`'s input: the caller whose identity to report. How `user_id` was
/// established — a session, an identity file — is the driver's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeQuery {
    pub user_id: UserId,
}

/// Who the caller is, flattened for rendering: the recognized user's DID,
/// plus their public profile when it could be resolved. `profile` is
/// `None` when neither the cache nor the PDS answered — absence is not an
/// error (R4); the drivers render the bare DID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub id: UserId,
    pub profile: Option<MeProfile>,
}

/// The public-profile facts `me` surfaces. No `did` — it's on [`Output`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeProfile {
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<Profile> for MeProfile {
    fn from(profile: Profile) -> Self {
        MeProfile {
            handle: profile.handle,
            display_name: profile.display_name,
            avatar_url: profile.avatar_url,
        }
    }
}
/// Why [`me`] could not answer. `Display` is terse and never interpolates
/// the cause (a driver may print it); the cause stays on `source()`.
#[derive(Debug)]
pub enum MeError {
    /// No user carries `id` — a stale session or identity file.
    UnknownUser(UserId),
    /// The user store failed.
    Store(anyhow::Error),
}

impl From<anyhow::Error> for MeError {
    fn from(err: anyhow::Error) -> Self {
        Self::Store(err)
    }
}

impl std::fmt::Display for MeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeError::UnknownUser(id) => write!(f, "no user with id {id:?}"),
            MeError::Store(_) => write!(f, "the user store failed"),
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

/// `GET /me` and `zurfur session whoami`: load the user behind
/// [`MeQuery::user_id`] and resolve their profile read-through
/// ([`Profile::resolve_through`]).
impl Users<'_> {
    pub async fn me(
        &self,
        query: MeQuery,
        profile_cache: &dyn ProfileCache,
        profile_source: &dyn ProfileSource,
    ) -> Result<Output, MeError> {
        let ports = self.ports();
        let user = ports
            .users
            .find_by_did(&query.user_id)
            .await?
            .ok_or(MeError::UnknownUser(query.user_id))?;
        let profile = Profile::resolve_through(profile_cache, profile_source, &user.id).await;
        Ok(Output {
            id: user.id,
            profile: profile.map(MeProfile::from),
        })
    }
}
