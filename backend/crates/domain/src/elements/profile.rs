//! The [`Profile`] — a visitor's public, PDS-owned profile. It sits on the
//! public boundary, so the domain reads and caches it but never owns it:
//! fetched via [`crate::ports::ProfileSource`], cached behind
//! [`crate::ports::ProfileCache`].

use crate::elements::did::Did;
use crate::ports::{ProfileCache, ProfileSource};

/// A handle as the network reports it, carried for display only. Never
/// validated or verified here: the PDS is the authority, and `handle.invalid`
/// is a legitimate value. Not a `Handle`, which is a handle Zurfur has claimed.
///
/// ```
/// use domain::elements::profile::DisplayHandle;
///
/// let handle = DisplayHandle::from("alice.bsky.social");
/// assert_eq!(handle.to_string(), "alice.bsky.social");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayHandle(String);

impl From<String> for DisplayHandle {
    fn from(handle: String) -> Self {
        Self(handle)
    }
}

impl From<&str> for DisplayHandle {
    fn from(handle: &str) -> Self {
        Self(handle.to_string())
    }
}

impl AsRef<str> for DisplayHandle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DisplayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A visitor's public profile, read from their PDS. `display_name` and
/// `avatar_url` are optional — a PDS may carry neither, and a page must still
/// render from the handle alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub did: Did,
    pub handle: DisplayHandle,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl Profile {
    /// A profile with the two facts every PDS carries and nothing optional.
    pub fn new(did: Did, handle: impl Into<DisplayHandle>) -> Self {
        Self {
            did,
            handle: handle.into(),
            display_name: None,
            avatar_url: None,
        }
    }

    /// The same profile with a display name.
    pub fn with_display_name(self, display_name: impl Into<String>) -> Self {
        Self {
            display_name: Some(display_name.into()),
            ..self
        }
    }

    /// The same profile with an avatar URL.
    pub fn with_avatar_url(self, avatar_url: impl Into<String>) -> Self {
        Self {
            avatar_url: Some(avatar_url.into()),
            ..self
        }
    }

    /// Read-through resolution of a visitor's profile: a cache hit is served
    /// without waking the PDS, a miss reads the PDS and caches the result, and a
    /// PDS failure degrades to `None` rather than erroring.
    ///
    /// The cache fill is pool-backed and best-effort — a documented exception to
    /// the compile-enforced Unit of Work, and a `put` failure is
    /// swallowed so a cache hiccup never fails the read.
    pub async fn resolve_through(
        cache: &dyn ProfileCache,
        source: &dyn ProfileSource,
        did: &Did,
    ) -> Option<Profile> {
        if let Ok(Some(profile)) = cache.get(did).await {
            return Some(profile);
        }
        match source.fetch(did).await {
            Ok(profile) => {
                let _ = cache.put(&profile).await;
                Some(profile)
            }
            Err(_) => None,
        }
    }
}
