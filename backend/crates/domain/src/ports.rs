//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates (`adapter-pg`, `adapter-mem`). This is the first one; as the
//! `domain` crate splits into per-domain crates, `UserRepo` moves with the
//! `User` entity into the `identity` namespace.

use async_trait::async_trait;

use crate::elements::{
    did::Did,
    profile::Profile,
    user::{User, UserId},
};

/// Zurfur's record of recognized visitors. Identity precedes us, so this port
/// *recognizes* rather than registers (see ZMVP-9, DESIGN/User).
#[async_trait]
pub trait UserRepo: Send + Sync {
    /// Recognize a DID. The first call mints a User; every later call with the
    /// same DID returns that same User. One DID, one User, forever — idempotent,
    /// so callers needn't check existence first. (Criteria 1 & 2.)
    async fn provision(&self, did: &Did) -> anyhow::Result<User>;

    /// Resolve a session's stored UserId back to its User, without touching the
    /// PDS. Returns None if no such User exists. (Criterion 3.)
    async fn find(&self, id: UserId) -> anyhow::Result<Option<User>>;
}

/// Authenticates a visitor against their PDS, yielding the DID they already own
/// (the platform never mints one — identity precedes us). The two methods mirror
/// the OAuth handshake so the protocol library stays quarantined in `adapter-atproto`:
/// callers see only a handle, an opaque redirect URL, and a `Did`.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Begin sign-in for a handle; returns the PDS authorization URL to redirect
    /// the visitor to.
    async fn start(&self, handle: &str) -> anyhow::Result<String>;

    /// Complete the callback the PDS redirected back with, returning the
    /// authenticated visitor's DID. The query fields are the neutral ones a PDS
    /// may send (`code`/`state`/`iss`), never a protocol-library type.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
    ) -> anyhow::Result<Did>;
}

/// Reads a visitor's public profile from its source of truth — the user's PDS.
/// A public-boundary read (see DESIGN/"Domains and Applications"): async,
/// fallible, indexed reads may lag. The real adapter talks to the PDS; the mem
/// adapter fakes it. (ZMVP-10 criterion 1.)
#[async_trait]
pub trait ProfileSource: Send + Sync {
    /// Fetch the profile for a DID. The handle always resolves; `display_name`
    /// and `avatar_url` may be absent. Errors when the PDS is unreachable, so
    /// callers can degrade gracefully rather than fail the page.
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile>;
}

/// A private-side read-through cache of public profiles, so repeat views don't
/// need the PDS awake (ZMVP-10 criterion 2). Freshness/TTL policy lives in the
/// implementation; a caller treats a miss — absent or stale — as `None`.
#[async_trait]
pub trait ProfileCache: Send + Sync {
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>>;
    async fn put(&self, profile: &Profile) -> anyhow::Result<()>;
}
