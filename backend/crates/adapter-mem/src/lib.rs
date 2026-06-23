//! In-process fakes of the domain ports. Core development and tests run against
//! these so neither needs a database or a PDS (see CLAUDE.md, "adapter-mem").
//!
//! Each `Mem*` type implements one port from [`domain::ports`] entirely in
//! process: the private-store repos ([`MemUserRepo`], [`MemAccountRepo`],
//! [`MemProfileCache`]) back onto `HashMap`s behind a [`Mutex`]; the
//! public-boundary fakes ([`MemAuthenticator`], [`MemProfileSource`]) stand in
//! for the user's PDS; and [`MemDidMinter`] hands out synthetic account DIDs.
//! Together they let the `api` composition root wire a fully working backend
//! with no Docker, no network, and no PLC directory.
//!
//! **Fidelity, not realism.** A fake reproduces the *contract* a handler depends
//! on (idempotent recognition, soft-delete invisibility, cache hits) but skips
//! everything operational — TTLs, transactions, real keypairs. Where behavior
//! intentionally diverges from production it is called out on the item.
//!
//! **Locking discipline.** Mutable state sits behind a `std::sync::Mutex`, not a
//! `tokio::sync::Mutex`, because no `.await` is ever held across a guard: each
//! method takes the lock, does synchronous map work, and drops it before
//! returning. A poisoned lock is unrecoverable here, so every `.lock()` simply
//! `.expect()`s. Call counters use an [`AtomicUsize`] and need no lock.
//!
//! References: DESIGN/"Domains and Applications"; the per-port detail lives on
//! the trait docs in [`domain::ports`].

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    did::Did,
    profile::Profile,
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
};
use domain::ports::{AccountRepo, Authenticator, DidMinter, ProfileCache, ProfileSource, UserRepo};

/// In-memory [`UserRepo`]. Keyed by DID so `provision` is idempotent — the same
/// DID always resolves to the User minted on its first sign-in.
#[derive(Default)]
pub struct MemUserRepo {
    /// Every recognized visitor, keyed by their DID. The DID — not the
    /// [`UserId`] — is the natural key, which is what makes `provision`
    /// idempotent; `find` scans the values to resolve a [`UserId`].
    by_did: Mutex<HashMap<Did, User>>,
}

impl MemUserRepo {
    /// An empty repo — no visitors recognized yet.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl UserRepo for MemUserRepo {
    /// Idempotent per DID: the first call mints (via [`User::recognize`]) and
    /// inserts; later calls return the stored `User` untouched. `or_insert_with`
    /// makes the mint-or-return one atomic map operation under the lock.
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

    /// Read-only counterpart to `provision`: a miss returns `None` rather than
    /// minting a new `User`.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        // Read-only — a miss returns None rather than minting, unlike `provision`.
        let by_did = self.by_did.lock().expect("MemUserRepo mutex poisoned");
        Ok(by_did.get(did).cloned())
    }
}

/// In-memory [`Authenticator`]: stands in for the PDS so the full sign-in flow can
/// be driven without a network. `start` hands back a fixed callback URL and
/// `complete` always yields the configured DID — i.e. "the PDS authenticated this
/// visitor" — letting an e2e test exercise everything downstream of the handshake.
pub struct MemAuthenticator {
    /// The DID every `complete` resolves to — the visitor this fake pretends the
    /// PDS just authenticated. Fixed at construction.
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
    /// The profile handed back for any DID. `Some` while the fake PDS is
    /// reachable; `None` after [`MemProfileSource::set_unreachable`], which makes
    /// `fetch` error. Behind a [`Mutex`] only so `set_unreachable` can flip it
    /// through a shared `&self`.
    // `None` simulates an unreachable PDS — `fetch` errors instead of returning.
    profile: Mutex<Option<Profile>>,
    /// Count of `fetch` calls, read via [`MemProfileSource::fetch_count`] to
    /// prove a cache hit avoided a second source read. [`AtomicUsize`] so it
    /// needs no lock.
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
    /// Returns the configured profile and bumps the call counter; errors instead
    /// once the fake has been flipped unreachable. The DID is ignored — one
    /// profile stands in for all.
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
    /// Cached profiles keyed by DID. Entries never expire here — see the
    /// struct note on freshness.
    by_did: Mutex<HashMap<Did, Profile>>,
}

impl MemProfileCache {
    /// An empty cache.
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

/// The fields of an [`Account`] we keep behind the lock. `Account` is not `Clone`
/// (an aggregate root, not a value), so we store its parts and rebuild a fresh
/// `Account` on every `find` rather than clone the original.
struct StoredAccount {
    /// The account's sovereign `did:plc` (minted by [`MemDidMinter`] in the
    /// real founding flow).
    did: Did,
    /// The account's display name.
    name: AccountName,
    /// When the account was founded.
    created_at: domain::datetime::DateTimeUtc,
    /// When the account was last modified.
    updated_at: domain::datetime::DateTimeUtc,
    /// Soft-delete tombstone: `Some` hides the account from `find`, mirroring the
    /// pg adapter's `deleted_at IS NULL` filter. The row is kept, not dropped.
    deleted_at: Option<domain::datetime::DateTimeUtc>,
}

/// In-memory [`AccountRepo`]. An account and its founder's Owner membership are
/// minted together (ZMVP-14); `create` inserts both under one lock, standing in
/// for the single private-store transaction the real (pg) adapter runs.
#[derive(Default)]
pub struct MemAccountRepo {
    /// [`StoredAccount`] parts keyed by [`AccountId`]. Stored as parts because
    /// [`Account`] isn't `Clone`; `find` rebuilds a fresh `Account` from them.
    accounts: Mutex<HashMap<AccountId, StoredAccount>>,
    /// The [`Role`] each user holds in each account, keyed by `(account, user)`.
    /// A missing key means non-membership — that's what `role_of` returns as
    /// `None`. A separate map from `accounts`, so the two locks are independent;
    /// `create` takes them in turn (accounts, then memberships).
    memberships: Mutex<HashMap<(AccountId, UserId), Role>>,
}

impl MemAccountRepo {
    /// An empty repo — no accounts, no memberships.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AccountRepo for MemAccountRepo {
    /// Inserts the account and the owner's membership in turn. The two `HashMap`s
    /// sit behind separate locks, so this isn't truly atomic — it stands in for
    /// the real pg adapter's single private-store transaction, which tests don't
    /// stress for partial failure.
    async fn create(&self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        // No await is held across either lock, so std Mutexes are fine here.
        let mut accounts = self.accounts.lock().expect("MemAccountRepo mutex poisoned");
        accounts.insert(
            account.id,
            StoredAccount {
                did: account.did.clone(),
                name: account.name.clone(),
                created_at: account.created_at,
                updated_at: account.updated_at,
                deleted_at: account.deleted_at,
            },
        );

        let UserAccount(user, account_id, role) = owner;
        let mut memberships = self
            .memberships
            .lock()
            .expect("MemAccountRepo mutex poisoned");
        memberships.insert((*account_id, *user), role.clone());
        Ok(())
    }

    /// Rebuilds an [`Account`] from its stored parts (it isn't `Clone`). A
    /// soft-deleted account resolves to `None`, the same as one that never
    /// existed.
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>> {
        let accounts = self.accounts.lock().expect("MemAccountRepo mutex poisoned");
        Ok(accounts.get(&id).and_then(|stored| {
            // A soft-deleted account resolves to nothing, per the port contract.
            if stored.deleted_at.is_some() {
                return None;
            }
            Some(Account {
                id,
                did: stored.did.clone(),
                name: stored.name.clone(),
                created_at: stored.created_at,
                updated_at: stored.updated_at,
                deleted_at: stored.deleted_at,
            })
        }))
    }

    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>> {
        let memberships = self
            .memberships
            .lock()
            .expect("MemAccountRepo mutex poisoned");
        Ok(memberships.get(&(account, user)).cloned())
    }

    async fn grant_role(&self, member: &UserAccount) -> anyhow::Result<()> {
        // Upsert into the (account, user) -> role map: a fresh member is seated, an
        // existing one's role replaced — the in-memory mirror of the pg adapter's
        // `ON CONFLICT ... DO UPDATE`. Granting a role is how a user joins an
        // account (DESIGN/Roles); the role tree (`parent`) is deferred on the floor.
        let UserAccount(user, account_id, role) = member;
        let mut memberships = self
            .memberships
            .lock()
            .expect("MemAccountRepo mutex poisoned");
        memberships.insert((*account_id, *user), role.clone());
        Ok(())
    }

    async fn revoke_role(&self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        // Remove the membership — inverse of `grant_role`. Removing one that isn't
        // there is a no-op, mirroring the pg adapter's DELETE.
        let mut memberships = self
            .memberships
            .lock()
            .expect("MemAccountRepo mutex poisoned");
        memberships.remove(&(account, user));
        Ok(())
    }
}

/// In-memory [`DidMinter`] test fake: hands back a deterministic, unique-per-call
/// synthetic `did:plc:` value from an internal counter. No real keypair, PLC
/// genesis, or directory write — just enough shape (`did:plc:mem<n>`) for tests
/// downstream of minting to run without infrastructure.
#[derive(Default)]
pub struct MemDidMinter {
    /// Monotonic counter feeding the next DID's suffix; starts at 0, so the first
    /// mint is `did:plc:mem000000`. [`AtomicUsize`] keeps minting lock-free.
    next: AtomicUsize,
}

impl MemDidMinter {
    /// A minter whose first DID is `did:plc:mem000000`.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DidMinter for MemDidMinter {
    /// Hands back the next deterministic, unique synthetic DID
    /// (`did:plc:mem<n>`, zero-padded to six digits) and never fails — no
    /// keypair, PLC genesis, or directory write. Distinct from a *visitor's*
    /// recognized DID; this one is created on an account's behalf.
    async fn mint(&self) -> anyhow::Result<Did> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        Ok(Did::new(format!("did:plc:mem{n:06}")))
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

    fn user_id() -> UserId {
        UserId::new(uuid::Uuid::now_v7())
    }

    // Builds a live account directly. Repo tests exercise storage, so they don't go
    // through the founding invariant (`Account::open`, ZMVP-14 #1) — that path is
    // covered end-to-end by the api `accounts.rs` test, which drives `POST /accounts`.
    fn live_account(did_s: &str) -> Account {
        let now = Utc::now();
        Account {
            id: AccountId::new(uuid::Uuid::now_v7()),
            did: did(did_s),
            name: AccountName::try_new("Test Studio").unwrap(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    // Founding persists the account; `find` reads it back by id — same DID, and the
    // soft-delete tombstone is absent (a live account).
    #[tokio::test]
    async fn create_then_find_returns_the_account() {
        let repo = MemAccountRepo::new();
        let account = live_account("did:plc:acct");
        let (id, account_did, account_name) =
            (account.id, account.did.clone(), account.name.clone());
        let owner = UserAccount(user_id(), account.id, Role::Owner(None));

        repo.create(&account, &owner).await.unwrap();
        let found = repo.find(id).await.unwrap().expect("account present");

        assert_eq!(found.id, id);
        assert_eq!(found.did, account_did);
        assert_eq!(found.name, account_name); // the name round-trips
        assert_eq!(found.deleted_at, None);
    }

    // The founder's Owner membership is minted alongside the account — `role_of`
    // returns it for the (user, account) pair.
    #[tokio::test]
    async fn role_of_owner_returns_owner() {
        let repo = MemAccountRepo::new();
        let account = live_account("did:plc:acct");
        let owner_id = user_id();
        let owner = UserAccount(owner_id, account.id, Role::Owner(None));
        let account_id = account.id;

        repo.create(&account, &owner).await.unwrap();

        let role = repo.role_of(owner_id, account_id).await.unwrap();
        assert_eq!(role, Some(Role::Owner(None)));
    }

    // An account we never founded resolves to nothing.
    #[tokio::test]
    async fn find_unknown_account_returns_none() {
        let repo = MemAccountRepo::new();
        let account = live_account("did:plc:acct");
        let owner = UserAccount(user_id(), account.id, Role::Owner(None));
        repo.create(&account, &owner).await.unwrap();

        let other = live_account("did:plc:other");
        let found = repo.find(other.id).await.unwrap();

        assert_eq!(found.map(|a| a.id), None);
    }

    // Each mint yields a distinct DID — accounts never share a sovereign identity.
    #[tokio::test]
    async fn mint_returns_distinct_dids() {
        let minter = MemDidMinter::new();

        let first = minter.mint().await.unwrap();
        let second = minter.mint().await.unwrap();

        assert_ne!(first, second);
    }
}
