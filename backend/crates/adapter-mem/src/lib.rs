//! In-process fakes of the domain ports. Core development and tests run against
//! these so neither needs a database or a PDS (see CLAUDE.md, "adapter-mem").

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    did::Did,
    profile::Profile,
    user::{User, UserId},
};
use domain::ports::{Authenticator, ProfileCache, ProfileSource, UserRepo};

/// In-memory [`UserRepo`]. Keyed by DID so `provision` is idempotent — the same
/// DID always resolves to the User minted on its first sign-in.
#[derive(Default)]
pub struct MemUserRepo {
    by_did: Mutex<HashMap<Did, User>>,
}

impl MemUserRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl UserRepo for MemUserRepo {
    async fn provision(&self, did: &Did) -> anyhow::Result<User> {
        // No await is held across the lock, so the std Mutex is fine here.
        let mut by_did = self.by_did.lock().expect("MemUserRepo mutex poisoned");
        let user = by_did
            .entry(did.clone())
            .or_insert_with(|| User::recognize(did.clone(), Utc::now()));
        Ok(user.clone())
    }

    async fn find(&self, id: UserId) -> anyhow::Result<Option<User>> {
        let by_did = self.by_did.lock().expect("MemUserRepo mutex poisoned");
        Ok(by_did.values().find(|u| u.id == id).cloned())
    }
}

/// In-memory [`Authenticator`]: stands in for the PDS so the full sign-in flow can
/// be driven without a network. `start` hands back a fixed callback URL and
/// `complete` always yields the configured DID — i.e. "the PDS authenticated this
/// visitor" — letting an e2e test exercise everything downstream of the handshake.
pub struct MemAuthenticator {
    did: Did,
}

impl MemAuthenticator {
    /// Build a fake that authenticates every sign-in as `did`.
    pub fn new(did: Did) -> Self {
        Self { did }
    }
}

#[async_trait]
impl Authenticator for MemAuthenticator {
    async fn start(&self, _handle: &str) -> anyhow::Result<String> {
        // Any callback URL works; the test issues the callback request itself. The
        // `code` is opaque to the fake — `complete` ignores it.
        Ok("/signin-callback?code=test".to_string())
    }

    async fn complete(
        &self,
        _code: String,
        _state: Option<String>,
        _iss: Option<String>,
    ) -> anyhow::Result<Did> {
        Ok(self.did.clone())
    }
}

/// In-memory [`ProfileSource`]: stands in for the PDS read so the profile flow
/// can be exercised without a network. Returns a fixed profile, counts its calls
/// (so a test can prove a cache hit avoided a second fetch), and can be flipped
/// to "unreachable" to drive graceful-degradation tests.
pub struct MemProfileSource {
    // `None` simulates an unreachable PDS — `fetch` errors instead of returning.
    profile: Mutex<Option<Profile>>,
    fetches: AtomicUsize,
}

impl MemProfileSource {
    /// A source that returns `profile` for every DID.
    pub fn new(profile: Profile) -> Self {
        Self {
            profile: Mutex::new(Some(profile)),
            fetches: AtomicUsize::new(0),
        }
    }

    /// Flip the fake PDS to unreachable; subsequent `fetch` calls error.
    pub fn set_unreachable(&self) {
        *self
            .profile
            .lock()
            .expect("MemProfileSource mutex poisoned") = None;
    }

    /// How many times `fetch` has been called — lets a test assert the cache
    /// served a repeat view without a second source read.
    pub fn fetch_count(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileSource for MemProfileSource {
    async fn fetch(&self, _did: &Did) -> anyhow::Result<Profile> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        self.profile
            .lock()
            .expect("MemProfileSource mutex poisoned")
            .clone()
            .ok_or_else(|| anyhow::anyhow!("PDS unreachable (fake)"))
    }
}

/// In-memory [`ProfileCache`]: a plain DID-keyed map. Never expires — TTL is the
/// real (pg) cache's policy; tests control freshness by what they put in.
#[derive(Default)]
pub struct MemProfileCache {
    by_did: Mutex<HashMap<Did, Profile>>,
}

impl MemProfileCache {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ProfileCache for MemProfileCache {
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>> {
        let by_did = self.by_did.lock().expect("MemProfileCache mutex poisoned");
        Ok(by_did.get(did).cloned())
    }

    async fn put(&self, profile: &Profile) -> anyhow::Result<()> {
        let mut by_did = self.by_did.lock().expect("MemProfileCache mutex poisoned");
        by_did.insert(profile.did.clone(), profile.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn did(s: &str) -> Did {
        Did::new(s.to_string())
    }

    // Criterion 2 — "one DID, one User, forever". A repeat sign-in must find the
    // very same User: same id, same created_at. If the second call minted afresh,
    // either would differ (the id is a new UUIDv7, created_at a later instant).
    #[tokio::test]
    async fn provision_is_idempotent_per_did() {
        let repo = MemUserRepo::new();
        let d = did("did:plc:alice");

        let first = repo.provision(&d).await.unwrap();
        let second = repo.provision(&d).await.unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.did, d);
    }

    // Distinct DIDs are distinct Users — recognition is keyed by DID, never shared.
    #[tokio::test]
    async fn distinct_dids_get_distinct_users() {
        let repo = MemUserRepo::new();

        let alice = repo.provision(&did("did:plc:alice")).await.unwrap();
        let bob = repo.provision(&did("did:plc:bob")).await.unwrap();

        assert_ne!(alice.id, bob.id);
    }

    // Criterion 3 — a session resolves back to its User by id, no PDS round-trip.
    #[tokio::test]
    async fn find_returns_the_provisioned_user() {
        let repo = MemUserRepo::new();
        let provisioned = repo.provision(&did("did:plc:alice")).await.unwrap();

        let found = repo.find(provisioned.id).await.unwrap();

        assert_eq!(found, Some(provisioned));
    }

    // An id we never minted resolves to nothing — an expired or forged session id
    // greets no one.
    #[tokio::test]
    async fn find_unknown_id_returns_none() {
        let repo = MemUserRepo::new();
        repo.provision(&did("did:plc:alice")).await.unwrap();

        let found = repo.find(UserId::new(uuid::Uuid::now_v7())).await.unwrap();

        assert_eq!(found, None);
    }
}
