//! In-process fakes of the domain ports. Core development and tests run against
//! these so neither needs a database or a PDS (see CLAUDE.md, "adapter-mem").
//!
//! The private-store repos are split along the read/write line exactly as the
//! pg adapter is (DD `24150017`): one shared [`MemBackend`] owns the maps, the
//! read stores ([`MemUserStore`], [`MemAccountStore`], [`MemProfileCache`]) read
//! them off `&self`, and the *write* views are reachable only on a
//! [`MemUnitOfWork`] vended by [`MemDatabase`]. The maps live behind
//! `Arc<Mutex<…>>` shared by every store and view. The public-boundary fakes
//! ([`MemAuthenticator`], [`MemProfileSource`]) stand in for the user's PDS, and
//! [`MemDidMinter`] hands out synthetic account DIDs.
//!
//! **Fidelity, not realism.** A fake reproduces the *contract* a handler depends
//! on (idempotent recognition, soft-delete invisibility, cache hits) but skips
//! everything operational — TTLs, real keypairs. The unit of work, though, *does*
//! model transactional rollback (DD `24150017`): [`MemDatabase::begin`] snapshots
//! the domain maps into a private staging copy, the write views mutate only that
//! copy, and [`MemUnitOfWork::commit`] applies it back to the shared store —
//! **dropping the handle without committing discards the staged writes**, exactly
//! like pg's drop = rollback. So a forgotten `commit()` leaves nothing behind in
//! mem either (exercised by [`tests`], mirroring the pg rollback assertion), and an
//! uncommitted unit's writes are invisible to the shared read stores — as in pg,
//! where a pool read can't see another connection's open transaction. The
//! read-through profile cache is the one exception: its best-effort fill writes
//! straight to the shared store (a documented Unit-of-Work exemption), so it is
//! neither staged nor rolled back. Where behavior intentionally diverges from
//! production it is called out on the item.
//!
//! **Locking discipline.** Mutable state sits behind a `std::sync::Mutex`, not a
//! `tokio::sync::Mutex`, because no `.await` is ever held across a guard: each
//! method takes the lock, does synchronous map work, and drops it before
//! returning. A poisoned lock is unrecoverable here, so every `.lock()` simply
//! `.expect()`s. Call counters use an [`AtomicUsize`] and need no lock.
//!
//! References: DESIGN/"Domains and Applications"; the per-port detail lives on
//! the trait docs in [`domain::ports`].

mod public_records;
pub use public_records::MemPublicRecords;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    account_keys::AccountKeys,
    commission::{Commission, CommissionId, CommissionTitle, LifecycleStep, Visibility},
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId, InvitationState},
    plc_operation::PlcOperationRecord,
    profile::Profile,
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
};
use domain::ports::{
    AccountStore, AccountWrites, Authenticator, CommissionWrites, Database, DidMinter, HandleTaken,
    KeyStore, PlcOperationLog, ProfileCache, ProfileSource, UnitOfWork, UserStore, UserWrites,
};

/// The shared in-memory private store: every map behind its own `Arc<Mutex<…>>`
/// so the read stores, the [`MemDatabase`] factory, and the write views vended by
/// its [`MemUnitOfWork`] all observe the same state. Cloning a `MemBackend` (or
/// any store built from it) clones the `Arc`s, not the data — so a write through
/// one handle is seen through another.
#[derive(Clone, Default)]
pub struct MemBackend {
    /// Every recognized visitor, keyed by their DID — the natural key that makes
    /// `provision` idempotent. `find` scans the values to resolve a [`UserId`].
    users: Arc<Mutex<HashMap<Did, User>>>,
    /// [`StoredAccount`] parts keyed by [`AccountId`] (stored as parts because
    /// [`Account`] isn't `Clone`; `find` rebuilds a fresh `Account`).
    accounts: Arc<Mutex<HashMap<AccountId, StoredAccount>>>,
    /// The [`Role`] each user holds in each account, keyed by `(account, user)`. A
    /// missing key means non-membership — what `role_of` returns as `None`.
    memberships: Arc<Mutex<HashMap<(AccountId, UserId), Role>>>,
    /// [`StoredInvitation`] parts keyed by [`InvitationId`] (ZMVP-32). The
    /// at-most-one-*pending*-per-(account, user) rule is enforced by scanning for an
    /// existing pending offer before inserting (the pg adapter uses a partial index).
    invitations: Arc<Mutex<HashMap<InvitationId, StoredInvitation>>>,
    /// Cached profiles keyed by DID. Entries never expire here — TTL is the real
    /// (pg) cache's policy; tests control freshness by what they put in.
    profiles: Arc<Mutex<HashMap<Did, Profile>>>,
    /// Append-only handle-change audit log (ZMVP-46), the in-memory mirror of the pg
    /// `account_handle_changes` table. Backs both the change rate limit and the
    /// vacated-handle quarantine (DD `27852802` §3/§4). A domain map, so it is staged
    /// and applied by the Unit of Work exactly like `accounts`/`memberships`.
    handle_changes: Arc<Mutex<Vec<StoredHandleChange>>>,
    /// [`StoredCommission`] parts keyed by [`CommissionId`] (ZMVP-65). Stored as
    /// parts because [`Commission`] isn't `Clone`; a read rebuilds a fresh
    /// `Commission`. Mirrors the pg `commission` table.
    commissions: Arc<Mutex<HashMap<CommissionId, StoredCommission>>>,
}

impl MemBackend {
    /// An empty backend — no visitors, accounts, memberships, invitations, or
    /// cached profiles.
    pub fn new() -> Self {
        Self::default()
    }

    /// The [`UserStore`] read port over this backend's shared state.
    pub fn user_store(&self) -> Arc<dyn UserStore> {
        Arc::new(MemUserStore(self.clone()))
    }

    /// The [`AccountStore`] read port over this backend's shared state.
    pub fn account_store(&self) -> Arc<dyn AccountStore> {
        Arc::new(MemAccountStore(self.clone()))
    }

    /// The [`ProfileCache`] read port over this backend's shared state.
    pub fn profile_cache(&self) -> Arc<dyn ProfileCache> {
        Arc::new(MemProfileCache(self.clone()))
    }

    /// The [`Database`] write factory over this backend's shared state.
    pub fn database(&self) -> Arc<dyn Database> {
        Arc::new(MemDatabase(self.clone()))
    }

    /// Snapshot the **domain** maps into a fresh staging backend for a unit of work
    /// (DD `24150017`): the user/account/membership/invitation maps are *deep*-copied
    /// into new `Arc<Mutex<…>>`, so writes through the unit mutate only the copy until
    /// [`MemUnitOfWork::commit`] applies it back. The profile cache map is *shared*
    /// (its `Arc` is cloned, not copied) — the cache fill is a documented Unit-of-Work
    /// exemption that writes straight through, so a unit must neither stage nor clobber
    /// it. Dropping the staged backend without applying it is the mem mirror of pg's
    /// rollback-on-drop.
    fn stage(&self) -> MemBackend {
        MemBackend {
            users: Arc::new(Mutex::new(
                self.users
                    .lock()
                    .expect("MemBackend users mutex poisoned")
                    .clone(),
            )),
            accounts: Arc::new(Mutex::new(
                self.accounts
                    .lock()
                    .expect("MemBackend accounts mutex poisoned")
                    .clone(),
            )),
            memberships: Arc::new(Mutex::new(
                self.memberships
                    .lock()
                    .expect("MemBackend memberships mutex poisoned")
                    .clone(),
            )),
            invitations: Arc::new(Mutex::new(
                self.invitations
                    .lock()
                    .expect("MemBackend invitations mutex poisoned")
                    .clone(),
            )),
            // Shared, not copied: the profile cache is a Unit-of-Work exemption.
            profiles: self.profiles.clone(),
            handle_changes: Arc::new(Mutex::new(
                self.handle_changes
                    .lock()
                    .expect("MemBackend handle_changes mutex poisoned")
                    .clone(),
            )),
            commissions: Arc::new(Mutex::new(
                self.commissions
                    .lock()
                    .expect("MemBackend commissions mutex poisoned")
                    .clone(),
            )),
        }
    }

    /// Apply a staged unit's domain maps onto this (shared) backend, replacing their
    /// contents wholesale — the mem mirror of a pg `COMMIT`. The profile map is left
    /// untouched (it was never staged). Only [`MemUnitOfWork::commit`] calls this; a
    /// dropped, un-applied unit leaves the shared store exactly as it was.
    fn apply(&self, staged: &MemBackend) {
        *self.users.lock().expect("MemBackend users mutex poisoned") = staged
            .users
            .lock()
            .expect("MemBackend users mutex poisoned")
            .clone();
        *self
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned") = staged
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned")
            .clone();
        *self
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned") = staged
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned")
            .clone();
        *self
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned") = staged
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned")
            .clone();
        *self
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned") = staged
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned")
            .clone();
        *self
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned") = staged
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned")
            .clone();
    }

    // --- Convenience seed/inspection helpers for tests. These operate directly on
    // the shared state (reusing the read/write impls) so a test can arrange and
    // assert without spelling out the `begin()`/accessor/`commit()` ceremony. ---

    /// Recognize a DID (seed/inspect a User). Idempotent, like the real
    /// [`UserWrites::provision`].
    pub async fn provision(&self, did: &Did) -> anyhow::Result<User> {
        MemUserWrites(self.clone()).provision(did).await
    }

    /// Resolve a DID to its User without minting one (inspect helper, the read-side
    /// counterpart to [`provision`](MemBackend::provision)).
    pub async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        MemUserStore(self.clone()).find_by_did(did).await
    }

    /// Found an account with its Owner membership (test seed of
    /// [`AccountWrites::create`]).
    pub async fn create(&self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        MemAccountWrites(self.clone()).create(account, owner).await
    }

    /// Seed a **soft-deleted** account holding `handle` (test-only). There is no
    /// soft-delete write path yet, so this inserts a tombstoned `StoredAccount`
    /// directly — the mem mirror of `UPDATE accounts SET deleted_at = …`. It lets a
    /// test assert that a tombstone (a) is invisible to resolution/`find` yet (b)
    /// still reserves its handle at founding, exactly as the global pg index does
    /// (DD `23003138`).
    pub fn seed_soft_deleted_account(&self, did: &Did, handle: &Handle) {
        let now = Utc::now();
        self.accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned")
            .insert(
                AccountId::new(uuid::Uuid::now_v7()),
                StoredAccount {
                    did: did.clone(),
                    handle: handle.clone(),
                    name: AccountName::try_new("Tombstoned").expect("valid name"),
                    created_at: now,
                    updated_at: now,
                    deleted_at: Some(now),
                },
            );
    }

    /// Seat/replace a member's role (test seed of [`AccountWrites::grant_role`]).
    pub async fn grant_role(&self, member: &UserAccount) -> anyhow::Result<()> {
        MemAccountWrites(self.clone()).grant_role(member).await
    }

    /// Issue a pending invitation (test seed of [`AccountWrites::create_invitation`]).
    pub async fn create_invitation(&self, invitation: &Invitation) -> anyhow::Result<()> {
        MemAccountWrites(self.clone())
            .create_invitation(invitation)
            .await
    }

    /// The role a user holds in an account, or `None` (inspect helper).
    pub async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>> {
        MemAccountStore(self.clone()).role_of(user, account).await
    }

    /// Resolve an account by id, or `None` if absent/soft-deleted (inspect helper).
    pub async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>> {
        MemAccountStore(self.clone()).find(id).await
    }

    /// The lone pending offer for `(account, invited)`, or `None` (inspect helper).
    pub async fn find_pending_invitation(
        &self,
        account: AccountId,
        invited: UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        MemAccountStore(self.clone())
            .find_pending_invitation(account, invited)
            .await
    }

    /// Resolve a commission by id, rebuilding it from its stored parts (it isn't
    /// `Clone`), or `None` if never created (inspect helper — there is no
    /// `CommissionStore` read port in the birth ticket, ZMVP-65, so tests read the
    /// shared store directly, as they do for the pre-read-port account seams).
    pub async fn find_commission(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        let commissions = self
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        Ok(commissions.get(&id).map(|stored| Commission {
            id,
            title: stored.title.clone(),
            owner_id: stored.owner_id,
            lifecycle_step: stored.lifecycle_step.clone(),
            visibility: stored.visibility.clone(),
            deadline: stored.deadline,
            created_at: stored.created_at,
        }))
    }

    /// Every stored commission, rebuilt from its parts, in unspecified order
    /// (inspect helper). Lets an api test that drives `POST /commissions` — which
    /// returns a bare `201` with no id — introspect what was persisted (owner,
    /// lifecycle) without a read port.
    pub async fn all_commissions(&self) -> anyhow::Result<Vec<Commission>> {
        let commissions = self
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        Ok(commissions
            .iter()
            .map(|(id, stored)| Commission {
                id: *id,
                title: stored.title.clone(),
                owner_id: stored.owner_id,
                lifecycle_step: stored.lifecycle_step.clone(),
                visibility: stored.visibility.clone(),
                deadline: stored.deadline,
                created_at: stored.created_at,
            })
            .collect())
    }
}

/// In-memory [`UserStore`] read surface over the shared [`MemBackend`].
pub struct MemUserStore(MemBackend);

#[async_trait]
impl UserStore for MemUserStore {
    async fn find(&self, id: UserId) -> anyhow::Result<Option<User>> {
        let users = self
            .0
            .users
            .lock()
            .expect("MemBackend users mutex poisoned");
        Ok(users.values().find(|u| u.id == id).cloned())
    }

    /// Read-only counterpart to `provision`: a miss returns `None` rather than
    /// minting a new `User`.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        let users = self
            .0
            .users
            .lock()
            .expect("MemBackend users mutex poisoned");
        Ok(users.get(did).cloned())
    }
}

/// In-memory [`UserWrites`] view: recognition writes onto the shared state. Vended
/// only by [`MemUnitOfWork::users`] in production wiring (tests reach it via
/// [`MemBackend::provision`]).
pub struct MemUserWrites(MemBackend);

#[async_trait]
impl UserWrites for MemUserWrites {
    /// Idempotent per DID: the first call mints (via [`User::recognize`]) and
    /// inserts; later calls return the stored `User` untouched. `or_insert_with`
    /// makes the mint-or-return one atomic map operation under the lock.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User> {
        let mut users = self
            .0
            .users
            .lock()
            .expect("MemBackend users mutex poisoned");
        let user = users
            .entry(did.clone())
            .or_insert_with(|| User::recognize(did.clone(), Utc::now()));
        Ok(user.clone())
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

/// In-memory [`ProfileCache`] over the shared [`MemBackend`]: a plain DID-keyed
/// map. Never expires — TTL is the real (pg) cache's policy; tests control
/// freshness by what they put in. Both `get` and the best-effort `put` are `&self`
/// here, mirroring the pg adapter: the cache fill is a documented exception to the
/// Unit of Work, not a write view.
pub struct MemProfileCache(MemBackend);

#[async_trait]
impl ProfileCache for MemProfileCache {
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>> {
        let profiles = self
            .0
            .profiles
            .lock()
            .expect("MemBackend profiles mutex poisoned");
        Ok(profiles.get(did).cloned())
    }

    async fn put(&self, profile: &Profile) -> anyhow::Result<()> {
        let mut profiles = self
            .0
            .profiles
            .lock()
            .expect("MemBackend profiles mutex poisoned");
        profiles.insert(profile.did.clone(), profile.clone());
        Ok(())
    }
}

/// The fields of an [`Account`] we keep behind the lock. `Account` is not `Clone`
/// (an aggregate root, not a value), so we store its parts and rebuild a fresh
/// `Account` on every `find` rather than clone the original. `Clone` so a unit of
/// work can deep-copy the accounts map into its staging snapshot (see
/// [`MemBackend::stage`]).
#[derive(Clone)]
struct StoredAccount {
    /// The account's sovereign `did:plc` (minted by [`MemDidMinter`] in the
    /// real founding flow).
    did: Did,
    /// The account's public handle — the validated, normalized name it is reached
    /// by, globally unique (a soft-deleted account still reserves it, DD/23003138;
    /// mirrors the pg `handle` column + its `accounts_handle_key` index).
    handle: Handle,
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

/// The fields of an [`Invitation`] we keep behind the lock. Like [`Account`],
/// `Invitation` isn't `Clone` (an entity with a lifecycle, not a value), so we
/// store its parts and rebuild a fresh `Invitation` on read. `Clone` so a unit of
/// work can deep-copy the invitations map into its staging snapshot (see
/// [`MemBackend::stage`]).
#[derive(Clone)]
struct StoredInvitation {
    /// The account membership is being offered of.
    account: AccountId,
    /// The User being invited.
    invited_user: UserId,
    /// The offered rank.
    role: Role,
    /// The member who issued the offer (becomes the new member's Parent on
    /// acceptance — DESIGN/Roles rule 4a).
    inviter: UserId,
    /// Where the offer sits in its lifecycle. [`InvitationState`] is `Copy`.
    state: InvitationState,
    /// When the invitation was issued.
    created_at: domain::datetime::DateTimeUtc,
    /// When the invitation last changed state.
    updated_at: domain::datetime::DateTimeUtc,
}

/// One appended handle change as the mem backend keeps it — the in-memory mirror of a
/// pg `account_handle_changes` row (ZMVP-46). `Clone` so a unit of work can deep-copy
/// the log into its staging snapshot (see [`MemBackend::stage`]).
/// The mem fake keeps only the fields its reads consume — the account, the vacated
/// `old_handle`, and the instant. It deliberately drops the pg row's `new_handle`
/// (audit-only, read by nothing here), per this module's "fidelity, not realism" note.
#[derive(Clone)]
struct StoredHandleChange {
    /// The account whose handle changed.
    account_id: AccountId,
    /// The handle vacated by this change — what the quarantine reserves.
    old_handle: Handle,
    /// When the change committed — the rate-limit / quarantine window anchor.
    changed_at: DateTimeUtc,
}

/// The fields of a [`Commission`] we keep behind the lock. Like [`Account`],
/// `Commission` isn't `Clone` (an aggregate root, not a value), so we store its
/// parts and rebuild a fresh `Commission` on read. `Clone` so a unit of work can
/// deep-copy the commissions map into its staging snapshot (see
/// [`MemBackend::stage`]).
#[derive(Clone)]
struct StoredCommission {
    /// The commission's fixed, always-present Title (ZMVP-65), validated non-empty.
    title: CommissionTitle,
    /// The User who created it and owns it — the permanent owner (DESIGN/Commission).
    owner_id: UserId,
    /// Its single [`LifecycleStep`]; a freshly created commission is `Draft`.
    lifecycle_step: LifecycleStep,
    /// Who may see it; a freshly created commission is [`Visibility::Private`].
    visibility: Visibility,
    /// The nullable-but-fixed deadline envelope field.
    deadline: Option<domain::datetime::DateTimeUtc>,
    /// When the commission was created.
    created_at: domain::datetime::DateTimeUtc,
}

/// In-memory [`CommissionWrites`] view: commission writes land on the shared
/// state. Vended by [`MemUnitOfWork::commissions`], where the [`MemBackend`] it
/// wraps is the unit's *staging* snapshot — so a write reaches the shared store
/// only on [`MemUnitOfWork::commit`] (drop = rollback), exactly like
/// [`MemAccountWrites`].
pub struct MemCommissionWrites(MemBackend);

#[async_trait]
impl CommissionWrites for MemCommissionWrites {
    /// Insert the freshly created commission, keyed by its id — the in-memory
    /// mirror of the pg adapter's single `INSERT INTO commission`. The pg `id` is a
    /// PRIMARY KEY, so a duplicate would raise a violation there; the fake does not
    /// model that (a plain `insert`, the same as [`MemAccountWrites::create`] does
    /// for its own account id), because commission ids are freshly-minted UUIDv7 —
    /// a collision is unreachable by construction, never a case a test can reach.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        commissions.insert(
            commission.id,
            StoredCommission {
                title: commission.title.clone(),
                owner_id: commission.owner_id,
                lifecycle_step: commission.lifecycle_step.clone(),
                visibility: commission.visibility.clone(),
                deadline: commission.deadline,
                created_at: commission.created_at,
            },
        );
        Ok(())
    }
}

/// In-memory [`AccountStore`] read surface over the shared [`MemBackend`].
pub struct MemAccountStore(MemBackend);

#[async_trait]
impl AccountStore for MemAccountStore {
    /// Rebuilds an [`Account`] from its stored parts (it isn't `Clone`). A
    /// soft-deleted account resolves to `None`, the same as one that never
    /// existed.
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>> {
        let accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        Ok(accounts.get(&id).and_then(|stored| {
            // A soft-deleted account resolves to nothing, per the port contract.
            if stored.deleted_at.is_some() {
                return None;
            }
            Some(Account {
                id,
                did: stored.did.clone(),
                handle: stored.handle.clone(),
                name: stored.name.clone(),
                created_at: stored.created_at,
                updated_at: stored.updated_at,
                deleted_at: stored.deleted_at,
            })
        }))
    }

    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>> {
        let memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        Ok(memberships.get(&(account, user)).cloned())
    }

    /// Scans for the lone pending offer for `(account, invited_user)`, rebuilding
    /// an [`Invitation`] from its parts (it isn't `Clone`). Accepted/revoked
    /// invitations are history, not live offers, so they never match.
    async fn find_pending_invitation(
        &self,
        account: AccountId,
        invited_user: UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        let invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        Ok(invitations.iter().find_map(|(id, stored)| {
            (stored.account == account
                && stored.invited_user == invited_user
                && stored.state == InvitationState::Pending)
                .then(|| rebuild_invitation(*id, stored))
        }))
    }

    /// Rebuilds the [`Invitation`] for `id` in whatever state it holds, or `None`.
    async fn find_invitation(&self, id: InvitationId) -> anyhow::Result<Option<Invitation>> {
        let invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        Ok(invitations
            .get(&id)
            .map(|stored| rebuild_invitation(id, stored)))
    }

    /// Scans for the live account whose handle matches, returning its `did`. A
    /// soft-deleted account resolves to `None`, mirroring `find` and the pg
    /// adapter's `deleted_at IS NULL` filter. `Handle` equality is exact (both
    /// sides are normalized), so this is the in-memory mirror of the unique-index
    /// lookup.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        let accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        Ok(accounts.values().find_map(|stored| {
            (stored.deleted_at.is_none() && &stored.handle == handle).then(|| stored.did.clone())
        }))
    }

    /// Counts this account's recorded changes at or after `since` — the in-memory
    /// mirror of the pg `count(*)` over `account_handle_changes` (ZMVP-46 rate limit).
    async fn count_handle_changes_since(
        &self,
        account: AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        let changes = self
            .0
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned");
        Ok(changes
            .iter()
            .filter(|change| change.account_id == account && change.changed_at >= since)
            .count() as i64)
    }

    /// Whether `handle` was recently vacated by an account other than `excluding` —
    /// the in-memory mirror of the pg `EXISTS` quarantine check (ZMVP-46 §4). A row for
    /// `excluding` itself never counts, so an account can reclaim its own vacated handle.
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool> {
        let changes = self
            .0
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned");
        Ok(changes.iter().any(|change| {
            &change.old_handle == handle
                && change.changed_at >= since
                && excluding.is_none_or(|account| change.account_id != account)
        }))
    }
}

/// In-memory [`AccountWrites`] view: account/membership/invitation writes. Vended by
/// [`MemUnitOfWork::accounts`], where the [`MemBackend`] it wraps is the unit's
/// *staging* snapshot — so writes land in the staged copy and reach the shared store
/// only on [`MemUnitOfWork::commit`] (drop = rollback). The test seed helpers on
/// [`MemBackend`] wrap the shared store directly, so they apply at once.
pub struct MemAccountWrites(MemBackend);

impl MemAccountWrites {
    /// Shared store effects of a member departing (`leave` / `revoke_role`): remove
    /// the membership and revoke the member's still-pending issued invitations. The
    /// mem fake doesn't model the role tree's `parent` (see `grant_role`), so there
    /// are no children to re-home — the pg adapter carries rule-3 re-homing.
    fn settle_member_departure(&self, user: UserId, account: AccountId) {
        self.0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned")
            .remove(&(account, user));

        let mut invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        for invitation in invitations.values_mut() {
            if invitation.account == account
                && invitation.inviter == user
                && matches!(invitation.state, InvitationState::Pending)
            {
                invitation.state = InvitationState::Revoked;
            }
        }
    }
}

#[async_trait]
impl AccountWrites for MemAccountWrites {
    /// Inserts the account and the owner's membership in turn. The two `HashMap`s
    /// sit behind separate locks, so this isn't truly atomic — it stands in for
    /// the real pg adapter's single private-store transaction, which tests don't
    /// stress for partial failure.
    ///
    /// Mirrors the pg `accounts_handle_key` unique index — **global**, spanning
    /// live *and* soft-deleted accounts (a tombstone reserves its handle, DD
    /// `23003138`): a handle already present in ANY state fails with [`HandleTaken`],
    /// the same typed error the handler maps to a `409`. Keeping this fidelity here
    /// lets the founding backstop (pre-check miss → store rejection) be exercised
    /// in-process.
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        let mut accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");

        // Global handle uniqueness — NOT filtered on `deleted_at`, unlike the read
        // path — so a soft-deleted account still reserves its handle.
        if accounts
            .values()
            .any(|stored| stored.handle == account.handle)
        {
            return Err(anyhow::Error::new(HandleTaken));
        }

        accounts.insert(
            account.id,
            StoredAccount {
                did: account.did.clone(),
                handle: account.handle.clone(),
                name: account.name.clone(),
                created_at: account.created_at,
                updated_at: account.updated_at,
                deleted_at: account.deleted_at,
            },
        );

        let UserAccount {
            user_id,
            account_id,
            role,
        } = owner;
        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        memberships.insert((*account_id, *user_id), role.clone());
        Ok(())
    }

    /// Repoints the stored account's handle to `new` and appends the change to the
    /// audit log — the in-memory mirror of the pg adapter's single transaction (ZMVP-46).
    /// `old` is an **optimistic-concurrency precondition** (matching the pg `handle =
    /// old` guard): the change applies only if the account is live and *still* holds
    /// `old`, else it fails and records no audit row — so a stale observation can't log a
    /// wrong `old_handle` (which would leave the truly vacated handle un-quarantined).
    /// The precondition is checked *before* uniqueness, mirroring the pg `UPDATE` (whose
    /// `WHERE handle = old` short-circuits ahead of the index). Global handle uniqueness
    /// across every OTHER account (live *or* tombstoned) then fails a collision with
    /// [`HandleTaken`] (→ 409), like [`create`](MemAccountWrites::create). Changing to
    /// the account's own current handle is a caller-side no-op rejected before this.
    async fn change_handle(
        &mut self,
        account: AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()> {
        let mut accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");

        // Precondition first (mirrors pg's `WHERE ... AND handle = old`, which gates the
        // row before the unique index is touched): the account must be live and still
        // hold `old`, else we roll back without auditing a stale change.
        if !accounts
            .get(&account)
            .is_some_and(|stored| stored.deleted_at.is_none() && &stored.handle == old)
        {
            anyhow::bail!(
                "change_handle: account {} is not a live account still holding the expected \
                 handle; nothing changed (concurrent change or removal)",
                *account
            );
        }

        // Global handle uniqueness across every OTHER account (live or tombstoned),
        // mirroring the pg `accounts_handle_key` index; the account's own row is exempt
        // (a no-op self-rename never reaches here).
        if accounts
            .iter()
            .any(|(id, stored)| *id != account && &stored.handle == new)
        {
            return Err(anyhow::Error::new(HandleTaken));
        }

        let stored = accounts
            .get_mut(&account)
            .expect("account presence checked by the precondition above");
        stored.handle = new.clone();
        stored.updated_at = at;
        drop(accounts);

        self.0
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned")
            .push(StoredHandleChange {
                account_id: account,
                old_handle: old.clone(),
                changed_at: at,
            });
        Ok(())
    }

    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()> {
        // Upsert into the (account, user) -> role map: a fresh member is seated, an
        // existing one's role replaced — the in-memory mirror of the pg adapter's
        // `ON CONFLICT ... DO UPDATE`. Granting a role is how a user joins an
        // account (DESIGN/Roles); the role tree (`parent`) is deferred on the floor.
        let UserAccount {
            user_id: user,
            account_id,
            role,
        } = member;
        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        memberships.insert((*account_id, *user), role.clone());
        Ok(())
    }

    async fn revoke_role(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        // A revoke is a member-departure event, identical to `leave` at the store
        // level (the caller settles authority): remove the membership and revoke the
        // member's pending issued invitations.
        self.settle_member_departure(user, account);
        Ok(())
    }

    async fn leave(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        // Self-removal (ZMVP-21); preconditions (membership exists, not Owner) are
        // the caller's. Same store effects as `revoke_role`.
        self.settle_member_departure(user, account);
        Ok(())
    }

    /// Inserts the pending invitation, unless one is already pending for the same
    /// `(account, invited_user)` — in which case this is a no-op, the in-memory
    /// mirror of the pg adapter's partial unique index (`... WHERE state =
    /// 'pending'`). The handler also checks `find_pending_invitation` first, so
    /// this is the belt-and-suspenders backstop, not the only guard.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<()> {
        let mut invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        let already_pending = invitations.values().any(|stored| {
            stored.account == invitation.account
                && stored.invited_user == invitation.invited_user
                && stored.state == InvitationState::Pending
        });
        if already_pending {
            // At most one pending offer per (account, user): a second issue is a
            // no-op, not a second row.
            return Ok(());
        }
        invitations.insert(
            invitation.id,
            StoredInvitation {
                account: invitation.account,
                invited_user: invitation.invited_user,
                role: invitation.role.clone(),
                inviter: invitation.inviter,
                state: invitation.state,
                created_at: invitation.created_at,
                updated_at: invitation.updated_at,
            },
        );
        Ok(())
    }

    /// Flips a pending invitation to revoked and stamps `updated_at`. A non-pending
    /// or absent invitation is left untouched — a no-op, not an error (the handler
    /// decides whether that's a 404/409), mirroring the pg adapter's guarded UPDATE.
    async fn revoke_invitation(&mut self, id: InvitationId) -> anyhow::Result<()> {
        let mut invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        if let Some(stored) = invitations.get_mut(&id)
            && stored.state == InvitationState::Pending
        {
            stored.state = InvitationState::Revoked;
            stored.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Flips the pending invitation to Accepted and seats the invited User as a
    /// member — the in-memory mirror of the pg adapter's single transaction, where
    /// the accepted state and the membership land together or not at all. The
    /// stored offer must still be pending; if it was accepted or revoked in the
    /// meantime (a lost race against the handler's `Invitation::accept` guard) this
    /// seats nothing and errors, honoring "a revoked invitation yields no
    /// membership". Like the pg guarded UPDATE, the *store's* state is what's
    /// checked — not the passed `invitation`, which the handler has already flipped.
    ///
    /// `parent` (the inviter, DESIGN/Roles rule 4a) and `listed_on_profile` are
    /// deferred on the floor here, exactly as the role tree is in `grant_role`: the
    /// pg adapter persists them in dedicated columns, but no port reads either back
    /// (`role_of` returns only the role), so the in-memory map keeps only the role.
    async fn accept_invitation(
        &mut self,
        invitation: Invitation,
        _listed_on_profile: bool,
    ) -> anyhow::Result<UserAccount> {
        {
            // The pending guard is the atomic backstop: matching no pending offer
            // means it was accepted or revoked since, so seat no member.
            let mut invitations = self
                .0
                .invitations
                .lock()
                .expect("MemBackend invitations mutex poisoned");
            match invitations.get_mut(&invitation.id) {
                Some(stored) if stored.state == InvitationState::Pending => {
                    stored.state = InvitationState::Accepted;
                    stored.updated_at = Utc::now();
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "invitation {} is no longer pending; no membership minted",
                        *invitation.id
                    ));
                }
            }
        }

        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        memberships.insert(
            (invitation.account, invitation.invited_user),
            invitation.role.clone(),
        );

        Ok(UserAccount {
            account_id: invitation.account,
            user_id: invitation.invited_user,
            role: invitation.role,
        })
    }

    /// Transfer ownership (DESIGN/Roles rule 8): promote the incoming member to the
    /// sole `Owner` and demote the outgoing `Owner` to `Admin`, in one lock so both
    /// role writes land together — the in-memory mirror of the pg adapter's single
    /// transaction. The caller (the handler) has already settled authority; the two
    /// guards here are the defensive backstop the pg adapter's `SELECT`s are, erroring
    /// (rather than half-transferring) if either membership vanished since that check.
    ///
    /// `parent` re-homing (the outgoing Owner under the new Owner, rule 5) is deferred
    /// on the floor exactly as it is in `grant_role`/`accept_invitation`: no port reads
    /// `parent` back, so the in-memory map keeps only the role.
    async fn transfer_ownership(
        &mut self,
        old_owner: UserId,
        new_owner: UserId,
        account: AccountId,
    ) -> anyhow::Result<()> {
        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");

        // Backstop: the outgoing Owner must still be the Owner of this account.
        if !matches!(memberships.get(&(account, old_owner)), Some(Role::Owner(_))) {
            return Err(anyhow::anyhow!(
                "user {} is not the Owner of account {}; ownership not transferred",
                *old_owner,
                *account
            ));
        }

        // Backstop: the incoming Owner must still be a member of this account.
        if !memberships.contains_key(&(account, new_owner)) {
            return Err(anyhow::anyhow!(
                "user {} is not a member of account {}; ownership not transferred",
                *new_owner,
                *account
            ));
        }

        memberships.insert((account, old_owner), Role::Admin(None));
        memberships.insert((account, new_owner), Role::Owner(None));
        Ok(())
    }

    /// Stamps `deleted_at` on the stored account (the in-memory mirror of pg's
    /// `UPDATE accounts SET deleted_at = now`), keeping the row so the handle stays
    /// reserved and reads (`find`/`find_did_by_handle`, which filter on `deleted_at`)
    /// treat it as absent. Memberships and invitations are left in place. Idempotent:
    /// an already-soft-deleted or absent account is a no-op. See the
    /// [`soft_delete`](AccountWrites::soft_delete) port doc.
    async fn soft_delete(&mut self, account: AccountId) -> anyhow::Result<()> {
        let mut accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        if let Some(stored) = accounts.get_mut(&account)
            && stored.deleted_at.is_none()
        {
            let now = Utc::now();
            stored.deleted_at = Some(now);
            stored.updated_at = now;
        }
        Ok(())
    }

    /// Removes the account row (freeing its handle for reuse) along with every
    /// membership and invitation belonging to it — the in-memory mirror of pg's
    /// children-first cascade delete. The custody keys are not modeled here. Removing
    /// an absent account is a no-op. See the [`hard_delete`](AccountWrites::hard_delete)
    /// port doc.
    async fn hard_delete(&mut self, account: AccountId) -> anyhow::Result<()> {
        self.0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned")
            .remove(&account);

        self.0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned")
            .retain(|(member_account, _), _| *member_account != account);

        self.0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned")
            .retain(|_, invitation| invitation.account != account);

        Ok(())
    }
}

/// In-memory [`Database`] write factory over the shared [`MemBackend`]. `begin`
/// snapshots the shared domain maps into a private staging backend and hands back a
/// [`MemUnitOfWork`] over it, so the unit's writes are isolated until it commits
/// (DD `24150017`; see the module note).
pub struct MemDatabase(MemBackend);

#[async_trait]
impl Database for MemDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        Ok(Box::new(MemUnitOfWork {
            shared: self.0.clone(),
            staged: self.0.stage(),
        }))
    }
}

/// In-memory [`UnitOfWork`] that models transactional rollback. Holds the `shared`
/// store and a private `staged` snapshot of its domain maps taken at `begin`; the
/// write views ([`accounts`](MemUnitOfWork::accounts)/[`users`](MemUnitOfWork::users))
/// mutate only `staged`. [`commit`](MemUnitOfWork::commit) applies `staged` back onto
/// `shared`; **dropping the handle without committing discards it** — the mem mirror
/// of pg's drop = rollback. Uncommitted writes are therefore invisible to the shared
/// read stores, matching pg (a pool read can't see another connection's open tx).
pub struct MemUnitOfWork {
    /// The real, shared store the unit commits back onto.
    shared: MemBackend,
    /// A private deep copy of the shared domain maps; the unit's writes land here
    /// and reach `shared` only on `commit`. (Shares the profile-cache `Arc` — that
    /// map is a documented Unit-of-Work exemption, never staged.)
    staged: MemBackend,
}

#[async_trait]
impl UnitOfWork for MemUnitOfWork {
    fn accounts(&mut self) -> Box<dyn AccountWrites + '_> {
        Box::new(MemAccountWrites(self.staged.clone()))
    }

    fn commissions(&mut self) -> Box<dyn CommissionWrites + '_> {
        Box::new(MemCommissionWrites(self.staged.clone()))
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        Box::new(MemUserWrites(self.staged.clone()))
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        // Apply the staged writes onto the shared store. Without this call the staged
        // snapshot is simply dropped, so the unit rolls back — as in pg.
        self.shared.apply(&self.staged);
        Ok(())
    }

    /// The mirror opposite of [`commit`](MemUnitOfWork::commit): commit *applies*
    /// the staged snapshot back onto the shared store, rollback simply does **not**.
    /// Consuming `self` here drops the staged copy, discarding every write in the
    /// unit — the same outcome as dropping the handle uncommitted, made explicit and
    /// deterministic (mem mirror of pg's awaited `ROLLBACK`).
    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Rebuilds an [`Invitation`] from its stored parts (it isn't `Clone`), the
/// invitation analogue of how `find` rebuilds an [`Account`].
fn rebuild_invitation(id: InvitationId, stored: &StoredInvitation) -> Invitation {
    Invitation {
        id,
        account: stored.account,
        invited_user: stored.invited_user,
        role: stored.role.clone(),
        inviter: stored.inviter,
        state: stored.state,
        created_at: stored.created_at,
        updated_at: stored.updated_at,
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
    /// keypair, PLC genesis, or directory write. `handle` is accepted to match
    /// the port (the real minter binds it into `alsoKnownAs`) but ignored here:
    /// the fake mints no real operation. Distinct from a *visitor's* recognized
    /// DID; this one is created on an account's behalf.
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        Ok(Did::new(format!("did:plc:mem{n:06}")))
    }

    /// No-op: the fake registers nothing, so there is nothing to tombstone. Matches
    /// the port so API-level deletion tests run without touching infrastructure (the
    /// real tombstone is exercised in the `RealDidMinter` unit tests).
    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
        Ok(())
    }

    /// No-op: the fake mints no real operation, so there is no `alsoKnownAs` to
    /// re-point. Matches the port so API-level handle-change tests (ZMVP-46) run
    /// without touching infrastructure (the real update is exercised in the
    /// `RealDidMinter` unit tests).
    async fn update_handle(&self, _did: &Did, _handle: &Handle) -> anyhow::Result<()> {
        Ok(())
    }
}

/// In-memory [`KeyStore`] test fake: holds custody keys in a process-local map,
/// **unencrypted** — safe only because they never leave memory and the fake
/// generates no real DID. Lets crates downstream of minting (and the
/// `RealDidMinter`'s own unit tests) exercise the put/get contract without a
/// database or a root key. The real at-rest encryption lives in the pg adapter.
#[derive(Clone, Default)]
pub struct MemKeyStore {
    /// DID string → its custody keys. `Arc<Mutex<…>>` so clones share state.
    keys: Arc<Mutex<HashMap<String, AccountKeys>>>,
}

impl MemKeyStore {
    /// An empty in-memory key store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KeyStore for MemKeyStore {
    /// Store `keys` under `did`. Per the [`KeyStore`] contract a DID mints once, so a
    /// second `put` for the same DID is **rejected** — mirroring the pg unique
    /// constraint on `account_keys.did`, so an accidental double-mint surfaces in
    /// tests instead of silently overwriting custody keys.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()> {
        let mut map = self.keys.lock().unwrap();
        if map.contains_key(did.as_str()) {
            anyhow::bail!("custody keys already exist for {}", did.as_str());
        }
        map.insert(did.to_string(), keys.clone());
        Ok(())
    }

    /// Return the custody keys for `did`, or `None` if never stored.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>> {
        Ok(self.keys.lock().unwrap().get(did.as_str()).cloned())
    }
}

/// One appended operation as [`MemPlcOperationLog`] keeps it — enough to mirror the
/// pg adapter's reads (`latest_cid`/`latest_op`) and its two integrity indexes.
#[derive(Clone)]
struct MemPlcEntry {
    did: String,
    cid: String,
    op_type: String,
    prev: Option<String>,
    operation_json: String,
}

/// In-memory [`PlcOperationLog`] test fake: keeps appended operations, in submission
/// order, in a process-local vec. Lets the `RealDidMinter`'s own unit tests exercise
/// the append / latest_cid / latest_op contract — chaining a tombstone or a handle
/// update onto the genesis op's CID — without a database.
#[derive(Clone, Default)]
pub struct MemPlcOperationLog {
    /// Appended entries in order; `Arc<Mutex<…>>` so clones share state.
    entries: Arc<Mutex<Vec<MemPlcEntry>>>,
}

impl MemPlcOperationLog {
    /// An empty in-memory operation log.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PlcOperationLog for MemPlcOperationLog {
    /// Append the operation in submission order, mirroring the pg adapter's two
    /// integrity indexes so tests catch a retry/fork bug instead of silently
    /// accepting it: a duplicate `cid` is **rejected** (`UNIQUE(cid)` — a
    /// content-addressed op is logged at most once), and a second non-genesis op
    /// chaining an already-used `prev` is **rejected** (`UNIQUE(did, prev)` where
    /// `prev IS NOT NULL` — the chain never forks; ZMVP-50 F1).
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        if entries.iter().any(|entry| entry.cid == record.cid) {
            anyhow::bail!("plc operation {} already logged", record.cid);
        }
        if let Some(prev) = &record.prev
            && entries.iter().any(|entry| {
                entry.did == record.did.as_str() && entry.prev.as_deref() == Some(prev)
            })
        {
            anyhow::bail!("plc operation already chains onto {prev} (chain would fork)");
        }
        entries.push(MemPlcEntry {
            did: record.did.to_string(),
            cid: record.cid.clone(),
            op_type: record.op_type.clone(),
            prev: record.prev.clone(),
            operation_json: record.operation_json.clone(),
        });
        Ok(())
    }

    /// The `cid` of the DID's most recently appended operation, or `None`.
    async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|entry| entry.did == did.as_str())
            .map(|entry| entry.cid.clone()))
    }

    /// The DID's most recently appended operation as a full record, or `None`.
    async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|entry| entry.did == did.as_str())
            .map(|entry| PlcOperationRecord {
                did: did.clone(),
                cid: entry.cid.clone(),
                op_type: entry.op_type.clone(),
                prev: entry.prev.clone(),
                operation_json: entry.operation_json.clone(),
            }))
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
        let backend = MemBackend::new();
        let d = did("did:plc:alice");

        let first = backend.provision(&d).await.unwrap();
        let second = backend.provision(&d).await.unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.did, d);
    }

    // Distinct DIDs are distinct Users — recognition is keyed by DID, never shared.
    #[tokio::test]
    async fn distinct_dids_get_distinct_users() {
        let backend = MemBackend::new();

        let alice = backend.provision(&did("did:plc:alice")).await.unwrap();
        let bob = backend.provision(&did("did:plc:bob")).await.unwrap();

        assert_ne!(alice.id, bob.id);
    }

    // Criterion 3 — a session resolves back to its User by id, no PDS round-trip.
    #[tokio::test]
    async fn find_returns_the_provisioned_user() {
        let backend = MemBackend::new();
        let provisioned = backend.provision(&did("did:plc:alice")).await.unwrap();

        let found = backend.user_store().find(provisioned.id).await.unwrap();

        assert_eq!(found, Some(provisioned));
    }

    // An id we never minted resolves to nothing — an expired or forged session id
    // greets no one.
    #[tokio::test]
    async fn find_unknown_id_returns_none() {
        let backend = MemBackend::new();
        backend.provision(&did("did:plc:alice")).await.unwrap();

        let found = backend
            .user_store()
            .find(UserId::new(uuid::Uuid::now_v7()))
            .await
            .unwrap();

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
        // Derive a valid, distinct handle from the did so accounts built for
        // different dids never collide on the unique handle.
        let label: String = did_s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        Account {
            id: AccountId::new(uuid::Uuid::now_v7()),
            did: did(did_s),
            handle: Handle::try_new(format!("{label}.example.com")).unwrap(),
            name: AccountName::try_new("Test Studio").unwrap(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    // The mem seam, end to end: a write issued through the UnitOfWork's account view
    // (begin → accounts().create → commit) is visible to a later read off the shared
    // AccountStore — proving the read store, the factory, and the write view share
    // state. Founding persists the account; `find` reads it back by id.
    #[tokio::test]
    async fn uow_create_is_visible_to_the_read_store() {
        let backend = MemBackend::new();
        let database = backend.database();
        let accounts = backend.account_store();

        let account = live_account("did:plc:acct");
        let (id, account_did, account_name) =
            (account.id, account.did.clone(), account.name.clone());
        let owner = UserAccount {
            user_id: user_id(),
            account_id: account.id,
            role: Role::Owner(None),
        };

        let mut uow = database.begin().await.unwrap();
        uow.accounts().create(&account, &owner).await.unwrap();
        uow.commit().await.unwrap();

        let found = accounts.find(id).await.unwrap().expect("account present");
        assert_eq!(found.id, id);
        assert_eq!(found.did, account_did);
        assert_eq!(found.name, account_name); // the name round-trips
        assert_eq!(found.deleted_at, None);
    }

    // Dropping a unit of work before `commit()` discards EVERY write in it — the mem
    // mirror of pg's drop = rollback (DD 24150017, mirroring the pg
    // `a_dropped_unit_of_work_rolls_back_every_write`). `create` stages two writes (the
    // account + the owner membership); once the handle drops uncommitted, neither
    // reaches the shared read store. This is what makes the rollback fidelity real and
    // exercised — a forgotten `commit()` now leaves nothing behind in mem too, so a mem
    // test can catch it.
    #[tokio::test]
    async fn a_dropped_unit_of_work_rolls_back_every_write() {
        let backend = MemBackend::new();
        let database = backend.database();
        let accounts = backend.account_store();

        let account = live_account("did:plc:rollback");
        let account_id = account.id;
        let owner_id = user_id();
        let owner = UserAccount {
            user_id: owner_id,
            account_id: account.id,
            role: Role::Owner(None),
        };

        // Open the unit, stage the (two-write) create, then drop WITHOUT committing.
        {
            let mut uow = database.begin().await.unwrap();
            uow.accounts().create(&account, &owner).await.unwrap();
            // `uow` drops here without `commit` → the staged writes are discarded.
        }

        assert!(
            accounts.find(account_id).await.unwrap().is_none(),
            "a dropped unit of work persists no account row"
        );
        assert_eq!(
            accounts.role_of(owner_id, account_id).await.unwrap(),
            None,
            "...and no membership either — both staged writes rolled back together"
        );
    }

    // An uncommitted unit's writes are invisible to a concurrent read off the shared
    // store *before* the unit commits — matching pg, where a pool read can't see
    // another connection's open transaction. Here the read store sees nothing until
    // `commit`, then sees the account.
    #[tokio::test]
    async fn uncommitted_writes_are_invisible_until_commit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let accounts = backend.account_store();

        let account = live_account("did:plc:isolated");
        let account_id = account.id;
        let owner = UserAccount {
            user_id: user_id(),
            account_id: account.id,
            role: Role::Owner(None),
        };

        let mut uow = database.begin().await.unwrap();
        uow.accounts().create(&account, &owner).await.unwrap();
        // Still open, not committed: the shared read store must not see it yet.
        assert!(
            accounts.find(account_id).await.unwrap().is_none(),
            "an open unit's staged write is invisible to a shared read"
        );

        uow.commit().await.unwrap();
        assert!(
            accounts.find(account_id).await.unwrap().is_some(),
            "the write becomes visible once the unit commits"
        );
    }

    // The founder's Owner membership is minted alongside the account — `role_of`
    // returns it for the (user, account) pair, read off the shared store.
    #[tokio::test]
    async fn role_of_owner_returns_owner() {
        let backend = MemBackend::new();
        let account = live_account("did:plc:acct");
        let owner_id = user_id();
        let owner = UserAccount {
            user_id: owner_id,
            account_id: account.id,
            role: Role::Owner(None),
        };
        let account_id = account.id;

        backend.create(&account, &owner).await.unwrap();

        let role = backend.role_of(owner_id, account_id).await.unwrap();
        assert_eq!(role, Some(Role::Owner(None)));
    }

    // An account we never founded resolves to nothing.
    #[tokio::test]
    async fn find_unknown_account_returns_none() {
        let backend = MemBackend::new();
        let account = live_account("did:plc:acct");
        let owner = UserAccount {
            user_id: user_id(),
            account_id: account.id,
            role: Role::Owner(None),
        };
        backend.create(&account, &owner).await.unwrap();

        let other = live_account("did:plc:other");
        let found = backend.find(other.id).await.unwrap();

        assert_eq!(found.map(|a| a.id), None);
    }

    // ZMVP-46 — the change flow's private half through the UnitOfWork: `change_handle`
    // repoints resolution (new resolves, old doesn't), and records the change so the
    // rate-limit count sees it. Staged like any account write, visible after commit.
    #[tokio::test]
    async fn change_handle_repoints_resolution_and_is_counted() {
        let backend = MemBackend::new();
        let database = backend.database();
        let store = backend.account_store();

        let account = live_account("did:plc:memchg");
        let (old, account_id, account_did) =
            (account.handle.clone(), account.id, account.did.clone());
        let owner = UserAccount {
            user_id: user_id(),
            account_id,
            role: Role::Owner(None),
        };
        backend.create(&account, &owner).await.unwrap();

        let new = Handle::try_new("memchg-new.example.com").unwrap();
        let mut uow = database.begin().await.unwrap();
        uow.accounts()
            .change_handle(account_id, &old, &new, Utc::now())
            .await
            .unwrap();
        uow.commit().await.unwrap();

        assert_eq!(
            store.find_did_by_handle(&new).await.unwrap(),
            Some(account_did),
            "the new handle resolves to the account's DID"
        );
        assert!(
            store.find_did_by_handle(&old).await.unwrap().is_none(),
            "the old handle no longer resolves"
        );
        assert_eq!(
            store
                .count_handle_changes_since(account_id, Utc::now() - chrono::Duration::minutes(5))
                .await
                .unwrap(),
            1,
            "the change is counted for the rate limit"
        );
    }

    // ZMVP-46 §4 — the vacated handle is quarantined to the leaving account: barred to
    // another account, excluded (reclaimable) for the account that left it.
    #[tokio::test]
    async fn change_handle_quarantines_the_vacated_handle() {
        let backend = MemBackend::new();
        let database = backend.database();
        let store = backend.account_store();

        let account = live_account("did:plc:memquar");
        let (old, account_id) = (account.handle.clone(), account.id);
        let owner = UserAccount {
            user_id: user_id(),
            account_id,
            role: Role::Owner(None),
        };
        backend.create(&account, &owner).await.unwrap();

        let new = Handle::try_new("memquar-new.example.com").unwrap();
        let mut uow = database.begin().await.unwrap();
        uow.accounts()
            .change_handle(account_id, &old, &new, Utc::now())
            .await
            .unwrap();
        uow.commit().await.unwrap();

        let window = Utc::now() - chrono::Duration::days(30);
        let stranger = AccountId::new(uuid::Uuid::now_v7());
        assert!(
            store
                .handle_reserved_for_other(&old, Some(stranger), window)
                .await
                .unwrap(),
            "the vacated handle is reserved against another account"
        );
        assert!(
            !store
                .handle_reserved_for_other(&old, Some(account_id), window)
                .await
                .unwrap(),
            "the leaving account may reclaim its own vacated handle"
        );
    }

    // Each mint yields a distinct DID — accounts never share a sovereign identity.
    #[tokio::test]
    async fn mint_returns_distinct_dids() {
        let minter = MemDidMinter::new();
        let handle = Handle::try_new("alice.zurfur.app").unwrap();

        let first = minter.mint(&handle).await.unwrap();
        let second = minter.mint(&handle).await.unwrap();

        assert_ne!(first, second);
    }

    // Parity with the real minter's port surface: the fake's update_handle is a
    // no-op that never fails (it registers no real operation), so API-level
    // handle-change tests run against mem without infrastructure.
    #[tokio::test]
    async fn mem_update_handle_is_a_noop() {
        let minter = MemDidMinter::new();
        let handle = Handle::try_new("alice.zurfur.app").unwrap();

        let did = minter.mint(&handle).await.unwrap();
        minter
            .update_handle(&did, &Handle::try_new("bob.zurfur.app").unwrap())
            .await
            .unwrap();
    }

    // The mem KeyStore round-trips custody keys: what you put is what you get.
    #[tokio::test]
    async fn mem_key_store_round_trips() {
        let store = MemKeyStore::new();
        let d = did("did:plc:alice");
        let keys = AccountKeys {
            cold_recovery: domain::elements::account_keys::SecretKey::new(vec![1u8; 32]),
            operational: domain::elements::account_keys::SecretKey::new(vec![2u8; 32]),
            signing: domain::elements::account_keys::SecretKey::new(vec![3u8; 32]),
        };

        assert!(store.get(&d).await.unwrap().is_none());
        store.put(&d, &keys).await.unwrap();
        assert_eq!(store.get(&d).await.unwrap().unwrap(), keys);
    }

    fn account_id() -> AccountId {
        AccountId::new(uuid::Uuid::now_v7())
    }

    // AC3 (store layer) — a freshly issued pending invitation round-trips: it is the
    // pending offer found for its (account, invited_user) pair, with every fact intact.
    #[tokio::test]
    async fn create_then_find_pending_returns_the_invitation() {
        let backend = MemBackend::new();
        let (account, invited, inviter) = (account_id(), user_id(), user_id());
        let invitation =
            Invitation::issue(account, invited, Role::Admin(None), inviter, Utc::now());
        let id = invitation.id;

        backend.create_invitation(&invitation).await.unwrap();

        let found = backend
            .account_store()
            .find_pending_invitation(account, invited)
            .await
            .unwrap()
            .expect("the pending invitation is found");
        assert_eq!(found.id, id);
        assert_eq!(found.role, Role::Admin(None));
        assert_eq!(found.inviter, inviter);
        assert_eq!(found.state, InvitationState::Pending);
    }

    // AC5 (store layer) — at most one pending per (account, user): a second issue for
    // the same pair while one is pending creates no second row.
    #[tokio::test]
    async fn a_second_pending_invitation_for_the_same_pair_is_not_a_second_row() {
        let backend = MemBackend::new();
        let (account, invited) = (account_id(), user_id());
        let first = Invitation::issue(account, invited, Role::Member(None), user_id(), Utc::now());
        let second = Invitation::issue(account, invited, Role::Admin(None), user_id(), Utc::now());

        backend.create_invitation(&first).await.unwrap();
        backend.create_invitation(&second).await.unwrap();

        // The original survives; the duplicate was a no-op (not a second row).
        let store = backend.account_store();
        let found = store
            .find_pending_invitation(account, invited)
            .await
            .unwrap()
            .expect("a pending invitation remains");
        assert_eq!(
            found.id, first.id,
            "the first pending offer is the one kept"
        );
        assert!(
            store.find_invitation(second.id).await.unwrap().is_none(),
            "the duplicate issue stored nothing"
        );
    }

    // AC4 (store layer) — revoking flips the offer to revoked: it is no longer the
    // pending offer for its pair, and a re-issue may now seat a fresh one. The revoke
    // goes through the UnitOfWork write view; the reads off the shared store.
    #[tokio::test]
    async fn revoke_invitation_flips_state_and_clears_the_pending_offer() {
        let backend = MemBackend::new();
        let database = backend.database();
        let store = backend.account_store();
        let (account, invited) = (account_id(), user_id());
        let invitation =
            Invitation::issue(account, invited, Role::Member(None), user_id(), Utc::now());
        let id = invitation.id;
        backend.create_invitation(&invitation).await.unwrap();

        let mut uow = database.begin().await.unwrap();
        uow.accounts().revoke_invitation(id).await.unwrap();
        uow.commit().await.unwrap();

        assert_eq!(
            store.find_invitation(id).await.unwrap().map(|i| i.state),
            Some(InvitationState::Revoked),
            "the invitation reads back revoked"
        );
        assert!(
            store
                .find_pending_invitation(account, invited)
                .await
                .unwrap()
                .is_none(),
            "a revoked invitation is no longer a live pending offer"
        );

        // With the prior offer revoked, a fresh invitation to the same pair is seated.
        let reissued =
            Invitation::issue(account, invited, Role::Admin(None), user_id(), Utc::now());
        backend.create_invitation(&reissued).await.unwrap();
        assert_eq!(
            store
                .find_pending_invitation(account, invited)
                .await
                .unwrap()
                .map(|i| i.id),
            Some(reissued.id),
            "re-inviting after a revoke seats a new pending offer"
        );
    }

    // An invitation id we never stored resolves to nothing.
    #[tokio::test]
    async fn find_unknown_invitation_returns_none() {
        let backend = MemBackend::new();
        let found = backend
            .account_store()
            .find_invitation(InvitationId::new(uuid::Uuid::now_v7()))
            .await
            .unwrap();
        assert!(found.is_none());
    }

    // ZMVP-65 AC1/AC2/AC3 (store layer) — a commission written through the
    // UnitOfWork's commission view (begin → commissions().create → commit) is read
    // back with its fixed metadata intact: the creating User is the owner and the
    // fresh commission is in `Draft`. The mem seam, end to end — proving the write
    // view and the shared store share state, mirroring the account seam test.
    #[tokio::test]
    async fn uow_create_commission_is_visible_after_commit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();

        let commission = Commission::create(
            CommissionTitle::try_new("A ref sheet").unwrap(),
            owner,
            Utc::now(),
            None,
        );
        let id = commission.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&commission).await.unwrap();
        uow.commit().await.unwrap();

        let found = backend
            .find_commission(id)
            .await
            .unwrap()
            .expect("commission present");
        assert_eq!(found.id, id);
        assert_eq!(found.title.as_str(), "A ref sheet");
        assert_eq!(found.owner_id, owner, "the creating User owns it");
        assert!(
            matches!(found.lifecycle_step, LifecycleStep::Draft),
            "a fresh commission is in Draft"
        );
        assert!(
            matches!(found.visibility, Visibility::Private),
            "a fresh commission is Private (the closed-door default)"
        );
    }

    // Dropping a unit of work before `commit()` discards the commission — the mem
    // mirror of pg's drop = rollback (DD 24150017), the commission analogue of
    // `a_dropped_unit_of_work_rolls_back_every_write`.
    #[tokio::test]
    async fn a_dropped_unit_of_work_rolls_back_the_commission() {
        let backend = MemBackend::new();
        let database = backend.database();

        let commission = Commission::create(
            CommissionTitle::try_new("Uncommitted").unwrap(),
            user_id(),
            Utc::now(),
            None,
        );
        let id = commission.id;

        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions().create(&commission).await.unwrap();
            // `uow` drops here without `commit` → the staged write is discarded.
        }

        assert!(
            backend.find_commission(id).await.unwrap().is_none(),
            "a dropped unit of work persists no commission row"
        );
    }

    // An uncommitted unit's commission is invisible to a read off the shared store
    // *before* the unit commits — matching pg, where a pool read can't see another
    // connection's open transaction.
    #[tokio::test]
    async fn uncommitted_commission_is_invisible_until_commit() {
        let backend = MemBackend::new();
        let database = backend.database();

        let commission = Commission::create(
            CommissionTitle::try_new("Isolated").unwrap(),
            user_id(),
            Utc::now(),
            None,
        );
        let id = commission.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&commission).await.unwrap();
        assert!(
            backend.find_commission(id).await.unwrap().is_none(),
            "an open unit's staged commission is invisible to a shared read"
        );

        uow.commit().await.unwrap();
        assert!(
            backend.find_commission(id).await.unwrap().is_some(),
            "the commission becomes visible once the unit commits"
        );
    }
}
