//! The [`Profile`] — a visitor's public, PDS-owned profile.
//!
//! Profile data sits on the public boundary (the user's PDS), so the domain
//! reads and caches it but never owns it (DESIGN/"Domains and Applications").
//! It is fetched via [`crate::ports::ProfileSource`] and cached behind
//! [`crate::ports::ProfileCache`] (ZMVP-10).

use crate::elements::did::Did;
use crate::ports::{ProfileCache, ProfileSource};

/// A visitor's public profile, read from their PDS. Handle, display name, and
/// avatar are user-owned data on the public boundary — we read and cache them,
/// we never own them. `display_name` and `avatar_url` are optional: a PDS may
/// carry neither, and the page must still render the handle (ZMVP-10's graceful
/// degradation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub did: Did,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl Profile {
    /// A profile with the two facts every PDS carries and nothing optional;
    /// see [`with_display_name`](Profile::with_display_name) and
    /// [`with_avatar_url`](Profile::with_avatar_url).
    pub fn new(did: Did, handle: impl Into<String>) -> Self {
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

    /// Read-through resolution of a visitor's profile: a fresh cache hit is
    /// served without waking the PDS (ZMVP-10 criterion 2); a miss reads the
    /// PDS and caches the result; a PDS failure degrades to `None` rather than
    /// erroring (criterion 3). One implementation for every driver — the HTTP
    /// `GET /me` and the CLI's `whoami` (ZMVP-203) both call this.
    ///
    /// The cache fill is pool-backed and best-effort — a documented exception
    /// to the compile-enforced Unit of Work (DD `24150017`): a read-through
    /// cache write on a read path has no transactional invariant, so it is not
    /// routed through a write transaction. A `put` failure is swallowed so a
    /// cache hiccup never fails the read.
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
