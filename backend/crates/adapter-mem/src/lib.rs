//! In-process fakes of the domain ports, so core development and tests need
//! neither a database nor a PDS.
//!
//! One shared [`MemBackend`] owns the maps; read stores read them off `&self`
//! and write views are reachable only on a [`MemUnitOfWork`] vended by
//! [`MemDatabase`], which stages a copy and merges it key-by-key on commit —
//! dropping without committing rolls back. Fidelity to the contract, not to
//! operational reality; intentional divergence is called out on the item.

mod actor_identity;
mod character;
mod commission;
mod file_store;
mod public_records;
mod workflow;
pub use actor_identity::{MemActorIdentityStore, MemActorIdentityWrites, StoredActorIdentity};
pub use character::{MemCharacterStore, MemCharacterWrites};
pub use commission::{
    MemChangelogStore, MemChangelogWrites, MemCommissionStore, MemCommissionWrites,
};
pub use file_store::MemFileStore;
pub use public_records::MemPublicRecords;
pub use workflow::{MemColumnStore, MemColumnWrites, MemWorkflowStore, MemWorkflowWrites};
use workflow::{StoredColumn, StoredWorkflow};

pub(crate) use commission::{
    StoredChangelogEntry, StoredCommission, StoredElement, StoredSeat, StoredSeatInvitation,
    StoredSlot, StoredTab,
};
pub(crate) use file_store::StoredBlob;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::{Account, AccountId, AccountMembership, AccountName, ListingScope},
    account_keys::AccountKeys,
    actor_identity::{ActorIdentityId, ActorKind, ActorState},
    character::{Character, CharacterId},
    commission::{
        CommissionFile, CommissionId, CommissionMarkup, ElementId, FileKey, GrantLevel, MarkupKey,
        SeatInvitationId, SurfaceName, TabId, VisibilityMode,
    },
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId, InvitationState},
    plc_operation::PlcOperationRecord,
    profile::Profile,
    role::{Role, RoleAlias},
    user::{User, UserId},
    user_account::UserAccount,
    workflow::{ColumnId, WorkflowId},
};
use domain::ports::DidBelongsToAnotherActor;
use domain::ports::character::CharacterWrites;
use domain::ports::{
    AccountReads, AccountRepo, AccountStore, AccountWrites, ActorIdentityStore,
    ActorIdentityWrites, Authenticator, ChangelogStore, ChangelogWrites, ColumnStore, ColumnWrites,
    CommissionRepo, CommissionStore, Database, DidMinter, FileStore, HandleTaken, KeyStore,
    PlcOperationLog, ProfileCache, ProfileSource, UnitOfWork, UserStore, UserWrites, WorkflowStore,
    WorkflowWrites,
};

/// The shared in-memory private store: every map behind its own `Arc<Mutex<…>>`,
/// so every store and view observes the same state. Cloning clones the `Arc`s,
/// not the data.
#[derive(Clone, Default)]
pub struct MemBackend {
    /// Every recognized visitor, keyed by DID — the key `provision` is
    /// idempotent on.
    users: Arc<Mutex<HashMap<Did, User>>>,
    /// [`StoredAccount`] parts keyed by [`AccountId`]; `find` rebuilds the
    /// `Account`, which is not `Clone`.
    accounts: Arc<Mutex<HashMap<AccountId, StoredAccount>>>,
    /// The [`StoredMembership`] each user holds in each account; a missing key
    /// means non-membership.
    memberships: Arc<Mutex<HashMap<(AccountId, UserId), StoredMembership>>>,
    /// [`StoredInvitation`] parts keyed by [`InvitationId`]; at most one pending
    /// offer per (account, user), enforced by scanning before insert.
    invitations: Arc<Mutex<HashMap<InvitationId, StoredInvitation>>>,
    /// Cached profiles keyed by DID; entries never expire here.
    profiles: Arc<Mutex<HashMap<Did, Profile>>>,
    /// Append-only handle-change audit log, backing the change rate limit and
    /// the vacated-handle quarantine. Staged.
    handle_changes: Arc<Mutex<Vec<StoredHandleChange>>>,
    /// [`StoredCommission`] parts keyed by [`CommissionId`]; a read rebuilds the
    /// `Commission`, which is not `Clone`.
    pub(crate) commissions: Arc<Mutex<HashMap<CommissionId, StoredCommission>>>,
    /// The commission changelog in append (= `seq`) order. Staged, so an entry
    /// commits atomically with the write it records; nothing ever mutates or
    /// removes a pushed entry.
    pub(crate) changelog: Arc<Mutex<Vec<StoredChangelogEntry>>>,
    /// Boards keyed by [`WorkflowId`]; one account each, columns in
    /// [`columns`](Self::columns).
    pub(crate) workflows: Arc<Mutex<HashMap<WorkflowId, StoredWorkflow>>>,
    /// Columns keyed by [`ColumnId`], each carrying its cards as one ordered
    /// list. A card IS a commission's placement, and the only place it lives.
    pub(crate) columns: Arc<Mutex<HashMap<ColumnId, StoredColumn>>>,
    /// Commission view grants keyed by `(commission, grantee DID)`. A pure key —
    /// just the level, with who/when in the changelog; at most one per pair, and
    /// a revoke hard-deletes it.
    pub(crate) view_grants: Arc<Mutex<HashMap<(CommissionId, Did), GrantLevel>>>,
    /// [`StoredElement`] parts keyed by [`ElementId`] — the commission's flat
    /// composition. Staged.
    pub(crate) elements: Arc<Mutex<HashMap<ElementId, StoredElement>>>,
    /// [`StoredTab`] parts keyed by [`TabId`]; staged, so a commission and its
    /// skeleton tabs commit together.
    pub(crate) tabs: Arc<Mutex<HashMap<TabId, StoredTab>>>,
    /// Per-commission surface-mode overrides, sparse: an absent entry means
    /// [`VisibilityMode::Total`].
    pub(crate) surface_modes: Arc<Mutex<HashMap<(CommissionId, SurfaceName), VisibilityMode>>>,
    /// Commission file-entry links keyed by [`FileKey`]; staged, so a link
    /// commits atomically with its `file_added` changelog entry.
    pub(crate) files: Arc<Mutex<HashMap<FileKey, CommissionFile>>>,
    /// Commission markups keyed by [`MarkupKey`]; staged alongside `files`.
    /// Immutable — the write port exposes no update and no delete.
    pub(crate) markups: Arc<Mutex<HashMap<MarkupKey, CommissionMarkup>>>,
    /// The file-entry blob store keyed by [`FileKey`], backing [`MemFileStore`].
    /// Shared, NOT staged: the blob write sits outside the Unit of Work.
    pub(crate) blobs: Arc<Mutex<HashMap<FileKey, StoredBlob>>>,
    /// [`StoredSlot`] satellites keyed by the carrying element's [`ElementId`];
    /// staged, so element and satellite commit or vanish together.
    pub(crate) slots: Arc<Mutex<HashMap<ElementId, StoredSlot>>>,
    /// Participant membership keyed by `(commission, user)` → when it began.
    /// Staged; there is no removal path at all, so the owner's row is permanent.
    pub(crate) participants: Arc<Mutex<HashMap<(CommissionId, UserId), DateTimeUtc>>>,
    /// [`StoredSeat`] parts keyed by the seat's [`ElementId`] — the interpreted
    /// half of the seat, sharing the element's id. Staged.
    pub(crate) seats: Arc<Mutex<HashMap<ElementId, StoredSeat>>>,
    /// [`StoredSeatInvitation`] parts keyed by [`SeatInvitationId`] — the owner's
    /// pending offer of a Seat. At most one pending per (seat, user), enforced by
    /// scanning before insert. Staged.
    pub(crate) seat_invitations: Arc<Mutex<HashMap<SeatInvitationId, StoredSeatInvitation>>>,
    /// [`StoredActorIdentity`] parts keyed by [`ActorIdentityId`]. Staged; rows
    /// are immortal — no write here removes one.
    pub(crate) actor_identities: Arc<Mutex<HashMap<ActorIdentityId, StoredActorIdentity>>>,
    /// Characters keyed by [`CharacterId`]; staged, so creation commits with
    /// the rest of the unit.
    pub(crate) characters: Arc<Mutex<HashMap<CharacterId, Character>>>,
}

impl MemBackend {
    /// An empty backend.
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

    /// The [`CommissionStore`] read port over this backend's shared state —
    /// the canonical commission read port's fake.
    pub fn commission_store(&self) -> Arc<dyn CommissionStore> {
        Arc::new(MemCommissionStore(self.clone()))
    }

    /// The [`ChangelogStore`] read port over this backend's shared state.
    pub fn changelog_store(&self) -> Arc<dyn ChangelogStore> {
        Arc::new(MemChangelogStore(self.clone()))
    }

    /// The [`WorkflowStore`] read port over this backend's shared state.
    pub fn workflow_store(&self) -> Arc<dyn WorkflowStore> {
        Arc::new(MemWorkflowStore(self.clone()))
    }

    /// The [`ColumnStore`] read port over this backend's shared state.
    pub fn column_store(&self) -> Arc<dyn ColumnStore> {
        Arc::new(MemColumnStore(self.clone()))
    }

    /// The [`ProfileCache`] read port over this backend's shared state.
    pub fn profile_cache(&self) -> Arc<dyn ProfileCache> {
        Arc::new(MemProfileCache(self.clone()))
    }

    /// The [`FileStore`] port over this backend's shared blob map.
    pub fn file_store(&self) -> Arc<dyn FileStore> {
        Arc::new(MemFileStore(self.clone()))
    }

    /// The [`Database`] write factory over this backend's shared state.
    pub fn database(&self) -> Arc<dyn Database> {
        Arc::new(MemDatabase(self.clone()))
    }

    /// The [`ActorIdentityStore`] read port over this backend's shared state.
    pub fn actor_identity_store(&self) -> Arc<dyn ActorIdentityStore> {
        Arc::new(MemActorIdentityStore(self.clone()))
    }

    /// Deep-copy the domain maps into a fresh staging backend, so a unit's
    /// writes mutate only the copy until [`MemUnitOfWork::commit`] applies it.
    /// The profile cache and blob maps are shared instead — Unit-of-Work
    /// exemptions that write straight through.
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
            changelog: Arc::new(Mutex::new(
                self.changelog
                    .lock()
                    .expect("MemBackend changelog mutex poisoned")
                    .clone(),
            )),
            workflows: Arc::new(Mutex::new(
                self.workflows
                    .lock()
                    .expect("MemBackend workflows mutex poisoned")
                    .clone(),
            )),
            columns: Arc::new(Mutex::new(
                self.columns
                    .lock()
                    .expect("MemBackend columns mutex poisoned")
                    .clone(),
            )),
            view_grants: Arc::new(Mutex::new(
                self.view_grants
                    .lock()
                    .expect("MemBackend view_grants mutex poisoned")
                    .clone(),
            )),
            elements: Arc::new(Mutex::new(
                self.elements
                    .lock()
                    .expect("MemBackend elements mutex poisoned")
                    .clone(),
            )),
            tabs: Arc::new(Mutex::new(
                self.tabs
                    .lock()
                    .expect("MemBackend tabs mutex poisoned")
                    .clone(),
            )),
            surface_modes: Arc::new(Mutex::new(
                self.surface_modes
                    .lock()
                    .expect("MemBackend surface_modes mutex poisoned")
                    .clone(),
            )),
            files: Arc::new(Mutex::new(
                self.files
                    .lock()
                    .expect("MemBackend files mutex poisoned")
                    .clone(),
            )),
            markups: Arc::new(Mutex::new(
                self.markups
                    .lock()
                    .expect("MemBackend markups mutex poisoned")
                    .clone(),
            )),
            // Shared, not copied: the blob store is a Unit-of-Work exemption.
            blobs: self.blobs.clone(),
            slots: Arc::new(Mutex::new(
                self.slots
                    .lock()
                    .expect("MemBackend slots mutex poisoned")
                    .clone(),
            )),
            participants: Arc::new(Mutex::new(
                self.participants
                    .lock()
                    .expect("MemBackend participants mutex poisoned")
                    .clone(),
            )),
            seats: Arc::new(Mutex::new(
                self.seats
                    .lock()
                    .expect("MemBackend seats mutex poisoned")
                    .clone(),
            )),
            seat_invitations: Arc::new(Mutex::new(
                self.seat_invitations
                    .lock()
                    .expect("MemBackend seat_invitations mutex poisoned")
                    .clone(),
            )),
            actor_identities: Arc::new(Mutex::new(
                self.actor_identities
                    .lock()
                    .expect("MemBackend actor_identities mutex poisoned")
                    .clone(),
            )),
            characters: Arc::new(Mutex::new(
                self.characters
                    .lock()
                    .expect("MemBackend characters mutex poisoned")
                    .clone(),
            )),
        }
    }

    /// Merge a unit's staged writes onto the shared backend key by key: `base`
    /// is the pristine snapshot taken at `begin`, and only keys whose staged
    /// value differs from it are written (or, if vanished, removed). A key this
    /// unit merely read merges as a no-op, so another unit's disjoint commit
    /// survives. `HashMap` fields go through [`merge_map`], append-log `Vec`s
    /// through [`merge_log`].
    ///
    /// Not modeled: a same-key write-write conflict is last-writer-wins here,
    /// where pg would serialize the second writer on a row lock.
    fn merge(&self, base: &MemBackend, staged: &MemBackend) {
        merge_map(
            &mut self.users.lock().expect("MemBackend users mutex poisoned"),
            &base.users.lock().expect("MemBackend users mutex poisoned"),
            &staged
                .users
                .lock()
                .expect("MemBackend users mutex poisoned"),
        );
        merge_map(
            &mut self
                .accounts
                .lock()
                .expect("MemBackend accounts mutex poisoned"),
            &base
                .accounts
                .lock()
                .expect("MemBackend accounts mutex poisoned"),
            &staged
                .accounts
                .lock()
                .expect("MemBackend accounts mutex poisoned"),
        );
        merge_map(
            &mut self
                .memberships
                .lock()
                .expect("MemBackend memberships mutex poisoned"),
            &base
                .memberships
                .lock()
                .expect("MemBackend memberships mutex poisoned"),
            &staged
                .memberships
                .lock()
                .expect("MemBackend memberships mutex poisoned"),
        );
        merge_map(
            &mut self
                .invitations
                .lock()
                .expect("MemBackend invitations mutex poisoned"),
            &base
                .invitations
                .lock()
                .expect("MemBackend invitations mutex poisoned"),
            &staged
                .invitations
                .lock()
                .expect("MemBackend invitations mutex poisoned"),
        );
        merge_log(
            &mut self
                .handle_changes
                .lock()
                .expect("MemBackend handle_changes mutex poisoned"),
            &base
                .handle_changes
                .lock()
                .expect("MemBackend handle_changes mutex poisoned"),
            &staged
                .handle_changes
                .lock()
                .expect("MemBackend handle_changes mutex poisoned"),
        );
        merge_map(
            &mut self
                .commissions
                .lock()
                .expect("MemBackend commissions mutex poisoned"),
            &base
                .commissions
                .lock()
                .expect("MemBackend commissions mutex poisoned"),
            &staged
                .commissions
                .lock()
                .expect("MemBackend commissions mutex poisoned"),
        );
        merge_log(
            &mut self
                .changelog
                .lock()
                .expect("MemBackend changelog mutex poisoned"),
            &base
                .changelog
                .lock()
                .expect("MemBackend changelog mutex poisoned"),
            &staged
                .changelog
                .lock()
                .expect("MemBackend changelog mutex poisoned"),
        );
        merge_map(
            &mut self
                .view_grants
                .lock()
                .expect("MemBackend view_grants mutex poisoned"),
            &base
                .view_grants
                .lock()
                .expect("MemBackend view_grants mutex poisoned"),
            &staged
                .view_grants
                .lock()
                .expect("MemBackend view_grants mutex poisoned"),
        );
        merge_map(
            &mut self
                .workflows
                .lock()
                .expect("MemBackend workflows mutex poisoned"),
            &base
                .workflows
                .lock()
                .expect("MemBackend workflows mutex poisoned"),
            &staged
                .workflows
                .lock()
                .expect("MemBackend workflows mutex poisoned"),
        );
        merge_map(
            &mut self
                .columns
                .lock()
                .expect("MemBackend columns mutex poisoned"),
            &base
                .columns
                .lock()
                .expect("MemBackend columns mutex poisoned"),
            &staged
                .columns
                .lock()
                .expect("MemBackend columns mutex poisoned"),
        );
        merge_map(
            &mut self
                .elements
                .lock()
                .expect("MemBackend elements mutex poisoned"),
            &base
                .elements
                .lock()
                .expect("MemBackend elements mutex poisoned"),
            &staged
                .elements
                .lock()
                .expect("MemBackend elements mutex poisoned"),
        );
        merge_map(
            &mut self.tabs.lock().expect("MemBackend tabs mutex poisoned"),
            &base.tabs.lock().expect("MemBackend tabs mutex poisoned"),
            &staged.tabs.lock().expect("MemBackend tabs mutex poisoned"),
        );
        merge_map(
            &mut self
                .surface_modes
                .lock()
                .expect("MemBackend surface_modes mutex poisoned"),
            &base
                .surface_modes
                .lock()
                .expect("MemBackend surface_modes mutex poisoned"),
            &staged
                .surface_modes
                .lock()
                .expect("MemBackend surface_modes mutex poisoned"),
        );
        merge_map(
            &mut self.files.lock().expect("MemBackend files mutex poisoned"),
            &base.files.lock().expect("MemBackend files mutex poisoned"),
            &staged
                .files
                .lock()
                .expect("MemBackend files mutex poisoned"),
        );
        merge_map(
            &mut self
                .markups
                .lock()
                .expect("MemBackend markups mutex poisoned"),
            &base
                .markups
                .lock()
                .expect("MemBackend markups mutex poisoned"),
            &staged
                .markups
                .lock()
                .expect("MemBackend markups mutex poisoned"),
        );
        merge_map(
            &mut self.slots.lock().expect("MemBackend slots mutex poisoned"),
            &base.slots.lock().expect("MemBackend slots mutex poisoned"),
            &staged
                .slots
                .lock()
                .expect("MemBackend slots mutex poisoned"),
        );
        merge_map(
            &mut self
                .participants
                .lock()
                .expect("MemBackend participants mutex poisoned"),
            &base
                .participants
                .lock()
                .expect("MemBackend participants mutex poisoned"),
            &staged
                .participants
                .lock()
                .expect("MemBackend participants mutex poisoned"),
        );
        merge_map(
            &mut self.seats.lock().expect("MemBackend seats mutex poisoned"),
            &base.seats.lock().expect("MemBackend seats mutex poisoned"),
            &staged
                .seats
                .lock()
                .expect("MemBackend seats mutex poisoned"),
        );
        merge_map(
            &mut self
                .seat_invitations
                .lock()
                .expect("MemBackend seat_invitations mutex poisoned"),
            &base
                .seat_invitations
                .lock()
                .expect("MemBackend seat_invitations mutex poisoned"),
            &staged
                .seat_invitations
                .lock()
                .expect("MemBackend seat_invitations mutex poisoned"),
        );
        merge_map(
            &mut self
                .actor_identities
                .lock()
                .expect("MemBackend actor_identities mutex poisoned"),
            &base
                .actor_identities
                .lock()
                .expect("MemBackend actor_identities mutex poisoned"),
            &staged
                .actor_identities
                .lock()
                .expect("MemBackend actor_identities mutex poisoned"),
        );
        merge_map(
            &mut self
                .characters
                .lock()
                .expect("MemBackend characters mutex poisoned"),
            &base
                .characters
                .lock()
                .expect("MemBackend characters mutex poisoned"),
            &staged
                .characters
                .lock()
                .expect("MemBackend characters mutex poisoned"),
        );
    }

    // --- Seed/inspection helpers for tests: they write straight to the shared
    // state, skipping the begin()/accessor/commit() ceremony. ---

    /// Recognize a DID (seed/inspect a User); idempotent.
    pub async fn provision(&self, did: &Did) -> anyhow::Result<User> {
        MemUserWrites(self.clone()).provision(did).await
    }

    /// Resolve a DID to its User without minting one (inspect helper).
    pub async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        MemUserStore(self.clone()).find_by_did(did).await
    }

    /// Found an account with its Owner membership (test seed).
    pub async fn create(&self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        MemAccountWrites(self.clone()).create(account, owner).await
    }

    /// Seed a soft-deleted account holding `handle` (test-only) by inserting a
    /// tombstoned row directly — there is no soft-delete write path yet.
    /// A tombstone is invisible to `find` but still reserves its handle.
    pub fn seed_soft_deleted_account(&self, did: &Did, handle: &Handle) {
        let now = Utc::now();
        self.accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned")
            .insert(
                AccountId::from(did.clone()),
                StoredAccount {
                    handle: handle.clone(),
                    name: "Tombstoned".parse::<AccountName>().expect("valid name"),
                    created_at: now,
                    updated_at: now,
                    deleted_at: Some(now),
                },
            );
    }

    /// Seat or replace a member's role (test seed).
    pub async fn grant_role(&self, member: &UserAccount) -> anyhow::Result<()> {
        MemAccountWrites(self.clone()).grant_role(member).await
    }

    /// Seed a member's [`RoleAlias`] onto an already-seated membership
    /// (test-only); there is no set-alias write path yet. Panics if `(account,
    /// user)` holds no membership.
    pub fn seed_role_alias(&self, user: UserId, account: AccountId, alias: RoleAlias) {
        self.memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned")
            .get_mut(&(account, user))
            .expect("seed_role_alias: no membership for (account, user)")
            .alias = Some(alias);
    }

    /// Issue a pending invitation (test seed).
    pub async fn create_invitation(&self, invitation: &Invitation) -> anyhow::Result<Invitation> {
        MemAccountWrites(self.clone())
            .create_invitation(invitation)
            .await
    }

    /// The role a user holds in an account, or `None` (inspect helper).
    pub async fn role_of(
        &self,
        user: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<Option<Role>> {
        MemAccountStore(self.clone()).role_of(user, account).await
    }

    /// Resolve an account by id, or `None` if absent/soft-deleted (inspect helper).
    pub async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        MemAccountStore(self.clone()).find(id).await
    }

    /// The lone pending offer for `(account, invited)`, or `None` (inspect helper).
    pub async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited: &UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        MemAccountStore(self.clone())
            .find_pending_invitation(account, invited)
            .await
    }

    /// How many blobs the file store currently holds (inspect helper), so a test
    /// can prove a rejected upload's blob was deleted rather than orphaned.
    pub fn blob_count(&self) -> usize {
        self.blobs
            .lock()
            .expect("MemBackend blobs mutex poisoned")
            .len()
    }

    // The commission seed/inspect helpers live with the commission fakes.
}

/// [`MemBackend::merge`]'s per-`HashMap` step: a key in `base` but gone from
/// `staged` is removed from `shared`; of the keys `staged` holds, only those
/// whose value differs from `base` are written. Everything else — an unchanged
/// key, or a key only `shared` knows — is left alone.
fn merge_map<K, V>(shared: &mut HashMap<K, V>, base: &HashMap<K, V>, staged: &HashMap<K, V>)
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone + PartialEq,
{
    for key in base.keys() {
        if !staged.contains_key(key) {
            shared.remove(key);
        }
    }
    for (key, value) in staged {
        if base.get(key) == Some(value) {
            continue; // untouched by this unit — leave shared's current state alone
        }
        shared.insert(key.clone(), value.clone());
    }
}

/// [`MemBackend::merge`]'s per-append-log step: the [`merge_map`] idea over a
/// `Vec`, with value equality standing in for a key — sound because a log row is
/// written once and never edited. New staged rows are pushed; rows this unit
/// removed are dropped.
fn merge_log<T: Clone + PartialEq>(shared: &mut Vec<T>, base: &[T], staged: &[T]) {
    for entry in base {
        if staged.contains(entry) {
            continue;
        }
        if let Some(position) = shared.iter().position(|row| row == entry) {
            shared.remove(position);
        }
    }
    for entry in staged {
        if base.contains(entry) || shared.contains(entry) {
            continue;
        }
        shared.push(entry.clone());
    }
}

/// In-memory [`UserStore`] read surface over the shared [`MemBackend`].
pub struct MemUserStore(MemBackend);

#[async_trait]
impl UserStore for MemUserStore {
    /// A direct lookup: a [`UserId`] IS the DID the map is keyed by.
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        let users = self
            .0
            .users
            .lock()
            .expect("MemBackend users mutex poisoned");
        Ok(users.get(&Did::from(id.to_string())).cloned())
    }

    /// Read-only counterpart to `provision`: a miss returns `None`.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        let users = self
            .0
            .users
            .lock()
            .expect("MemBackend users mutex poisoned");
        Ok(users.get(did).cloned())
    }
}

/// In-memory [`UserWrites`] view, vended only by [`MemUnitOfWork::users`].
pub struct MemUserWrites(MemBackend);

#[async_trait]
impl UserWrites for MemUserWrites {
    /// Recognize a DID in two steps: intern the identity row, then key the
    /// `users` projection by it. Idempotent per DID, so a repeat sign-in maps to
    /// the same User; both rows land in the same backend and commit together.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User> {
        let now = Utc::now();
        // Intern first: the identity row is the projection's parent.
        let identity = MemActorIdentityWrites(self.0.clone())
            .intern(did, ActorKind::User, now)
            .await?;
        // A DID already interned as another actor kind is a conflict, never a
        // silent reuse of that actor's identity id.
        if identity.kind != ActorKind::User {
            let conflict = DidBelongsToAnotherActor {
                existing_kind: identity.kind.as_str().to_string(),
            };
            return Err(anyhow::Error::new(conflict));
        }
        let mut users = self
            .0
            .users
            .lock()
            .expect("MemBackend users mutex poisoned");
        let user = users.entry(did.clone()).or_insert_with(|| User {
            id: UserId::from(did.clone()),
            created_at: now,
        });
        Ok(user.clone())
    }
}

/// In-memory [`Authenticator`]: `start` hands back a fixed callback URL and
/// `complete` always yields the configured DID, so the sign-in flow can be
/// driven without a network.
pub struct MemAuthenticator {
    /// The DID every `complete` resolves to, fixed at construction.
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
        // Any callback URL works; the test issues the callback itself.
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

/// In-memory [`ProfileSource`]: returns a fixed profile, counts its calls, and
/// can be flipped to unreachable for graceful-degradation tests.
pub struct MemProfileSource {
    /// The profile handed back for any DID; `None` after
    /// [`MemProfileSource::set_unreachable`] makes `fetch` error instead.
    profile: Mutex<Option<Profile>>,
    /// Count of `fetch` calls, read via [`MemProfileSource::fetch_count`].
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

    /// How many times `fetch` has been called.
    pub fn fetch_count(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileSource for MemProfileSource {
    /// Return the configured profile and bump the call counter; errors once the
    /// fake is unreachable. The DID is ignored.
    async fn fetch(&self, _did: &Did) -> anyhow::Result<Profile> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        self.profile
            .lock()
            .expect("MemProfileSource mutex poisoned")
            .clone()
            .ok_or_else(|| anyhow::anyhow!("PDS unreachable (fake)"))
    }
}

/// In-memory [`ProfileCache`] over the shared [`MemBackend`]: a DID-keyed map
/// that never expires. Both `get` and the best-effort `put` take `&self` — the
/// cache fill is a documented Unit-of-Work exemption, not a write view.
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

/// The fields of an [`Account`] we keep behind the lock; `find` rebuilds the
/// `Account`, which is not `Clone`. `Clone` lets a unit stage the map,
/// `PartialEq` lets [`merge_map`] tell an untouched row from a written one.
#[derive(Clone, PartialEq)]
struct StoredAccount {
    /// The account's public handle; globally unique, and a soft-deleted account
    /// still reserves it.
    handle: Handle,
    /// The account's display name.
    name: AccountName,
    /// When the account was founded.
    created_at: domain::datetime::DateTimeUtc,
    /// When the account was last modified.
    updated_at: domain::datetime::DateTimeUtc,
    /// Soft-delete tombstone: `Some` hides the account from `find`, keeping the
    /// row.
    deleted_at: Option<domain::datetime::DateTimeUtc>,
}

/// One `account_members` row: the [`Role`] held plus the member's
/// `listed_on_profile` choice, which is kept so the fake answers
/// [`ListingScope::PublicProfile`] exactly as pg does.
#[derive(Clone, PartialEq)]
struct StoredMembership {
    /// The role the member holds in the account.
    role: Role,
    /// The member's own alias for that role; no write path sets it yet.
    alias: Option<RoleAlias>,
    /// Whether the member publishes this membership; listed unless they say
    /// otherwise.
    listed_on_profile: bool,
}

impl StoredMembership {
    /// A membership seated with the defaults — listed, no alias.
    fn listed(role: Role) -> Self {
        Self {
            role,
            alias: None,
            listed_on_profile: true,
        }
    }
}

/// The fields of an [`Invitation`] we keep behind the lock; a read rebuilds the
/// `Invitation`, which is not `Clone`.
#[derive(Clone, PartialEq)]
struct StoredInvitation {
    /// The account membership is being offered of.
    account: AccountId,
    /// The User being invited.
    invited_user: UserId,
    /// The offered rank.
    role: Role,
    /// The member who issued the offer; the new member's Parent on acceptance.
    inviter: UserId,
    /// Where the offer sits in its lifecycle.
    state: InvitationState,
    /// When the invitation was issued.
    created_at: domain::datetime::DateTimeUtc,
    /// When the invitation last changed state.
    updated_at: domain::datetime::DateTimeUtc,
}

/// One appended handle change as the mem backend keeps it. Only the fields its
/// reads consume are stored — the pg row's audit-only `new_handle` is dropped.
/// `PartialEq` lets [`merge_log`] diff by value, since the row carries no id.
#[derive(Clone, PartialEq)]
struct StoredHandleChange {
    /// The account whose handle changed.
    account_id: AccountId,
    /// The handle vacated by this change; what the quarantine reserves.
    old_handle: Handle,
    /// When the change committed; the rate-limit and quarantine window anchor.
    changed_at: DateTimeUtc,
}

/// In-memory [`AccountStore`] read surface over the shared [`MemBackend`].
pub struct MemAccountStore(MemBackend);

#[async_trait]
impl AccountStore for MemAccountStore {
    /// Rebuild an [`Account`] from its stored parts; a soft-deleted account
    /// resolves to `None`.
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        let accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        Ok(accounts
            .get(id)
            .and_then(|stored| rebuild_account(id.clone(), stored)))
    }

    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>> {
        let memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        Ok(memberships
            .get(&(account.clone(), user.clone()))
            .map(|membership| membership.role.clone()))
    }

    /// Scan for the lone pending offer for `(account, invited_user)`; accepted
    /// and revoked invitations never match.
    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        let invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        Ok(invitations.iter().find_map(|(id, stored)| {
            (&stored.account == account
                && &stored.invited_user == invited_user
                && stored.state == InvitationState::Pending)
                .then(|| rebuild_invitation(*id, stored))
        }))
    }

    /// Rebuild the [`Invitation`] for `id` in whatever state it holds.
    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>> {
        let invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        Ok(invitations
            .get(id)
            .map(|stored| rebuild_invitation(*id, stored)))
    }

    /// Scan for the live account whose handle matches, returning its DID. A
    /// soft-deleted account resolves to `None`; handle equality is exact.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        let accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        Ok(accounts.iter().find_map(|(id, stored)| {
            (stored.deleted_at.is_none() && &stored.handle == handle).then(|| id.did().clone())
        }))
    }

    /// Count this account's recorded handle changes at or after `since`.
    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        let changes = self
            .0
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned");
        Ok(changes
            .iter()
            .filter(|change| &change.account_id == account && change.changed_at >= since)
            .count() as i64)
    }

    /// Whether `handle` was recently vacated by an account other than
    /// `excluding` — so an account can always reclaim its own vacated handle.
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
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
                && excluding.is_none_or(|account| &change.account_id != account)
        }))
    }

    /// Scan `memberships` for `user`'s rows, join each back to its account and
    /// drop the soft-deleted or vanished ones. Sorted by the account's DID, by
    /// byte value, so the result is deterministic.
    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>> {
        let accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        let memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");

        // A public projection honors the member's publication choice; their own
        // view ignores it.
        let honors_valve = matches!(scope, ListingScope::PublicProfile);

        let mut rows: Vec<AccountMembership> = memberships
            .iter()
            .filter(|((_, member_user), _)| member_user == user)
            .filter(|(_, membership)| !honors_valve || membership.listed_on_profile)
            .filter_map(|((account_id, _), membership)| {
                let stored = accounts.get(account_id)?;
                let account = rebuild_account(account_id.clone(), stored)?;
                Some(AccountMembership {
                    account,
                    role: membership.role.clone(),
                    alias: membership.alias.clone(),
                })
            })
            .collect();
        rows.sort_by(|left, right| {
            left.account
                .id
                .to_string()
                .cmp(&right.account.id.to_string())
        });
        Ok(rows)
    }
}

/// In-memory [`AccountWrites`] view, vended by [`MemUnitOfWork::accounts`] over
/// the unit's staging snapshot, so writes reach the shared store only on commit.
/// The test seed helpers wrap the shared store directly and apply at once.
pub struct MemAccountWrites(MemBackend);

impl MemAccountWrites {
    /// Shared store effects of a member departing: remove the membership and
    /// revoke their still-pending issued invitations. The mem fake models no role
    /// tree, so there are no children to re-home.
    fn settle_member_departure(&self, user: &UserId, account: &AccountId) {
        self.0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned")
            .remove(&(account.clone(), user.clone()));

        let mut invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        for invitation in invitations.values_mut() {
            if &invitation.account == account
                && &invitation.inviter == user
                && matches!(invitation.state, InvitationState::Pending)
            {
                invitation.state = InvitationState::Revoked;
            }
        }
    }
}

#[async_trait]
impl AccountWrites for MemAccountWrites {
    /// Insert the account and the owner's membership in turn — not truly atomic,
    /// standing in for pg's single transaction. Handle uniqueness is global across
    /// live AND soft-deleted accounts: a collision fails with [`HandleTaken`].
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        let mut accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");

        // NOT filtered on `deleted_at`, unlike the read path — so a soft-deleted
        // account still reserves its handle.
        if accounts
            .values()
            .any(|stored| stored.handle == account.handle)
        {
            return Err(anyhow::Error::new(HandleTaken));
        }

        // Intern the identity row BEFORE staging the projection, so a failure
        // leaves no partial account row.
        {
            let mut identities = self
                .0
                .actor_identities
                .lock()
                .expect("MemBackend actor_identities mutex poisoned");
            let existing = identities
                .values()
                .find(|stored| stored.did.as_ref() == Some(account.id.did()));
            match existing {
                Some(stored) => anyhow::ensure!(
                    stored.kind == ActorKind::Account,
                    "account DID {} is already interned as a different actor kind ({})",
                    account.id,
                    stored.kind.as_str()
                ),
                None => {
                    identities.insert(
                        ActorIdentityId::from(uuid::Uuid::now_v7()),
                        StoredActorIdentity {
                            kind: ActorKind::Account,
                            did: Some(account.id.did().clone()),
                            state: ActorState::Active,
                            handle: None,
                            first_seen: account.created_at,
                        },
                    );
                }
            }
        }

        accounts.insert(
            account.id.clone(),
            StoredAccount {
                handle: account.handle.clone(),
                name: account.name.clone(),
                created_at: account.created_at,
                updated_at: account.updated_at,
                deleted_at: account.deleted_at,
            },
        );
        drop(accounts);

        // The founder is seated with no alias; no write path seats one.
        let UserAccount {
            user_id,
            account_id,
            role,
            alias: _,
        } = owner;
        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        memberships.insert(
            (account_id.clone(), user_id.clone()),
            StoredMembership::listed(role.clone()),
        );
        Ok(())
    }

    /// Repoint the account's handle to `new` and append the change to the audit
    /// log. `old` is an optimistic-concurrency precondition checked first: the
    /// change applies only if the account is live and still holds `old`, else it
    /// fails and records no audit row. A collision with any OTHER account, live or
    /// tombstoned, then fails with [`HandleTaken`].
    async fn change_handle(
        &mut self,
        account: &AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()> {
        let mut accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");

        // Precondition first: the account must be live and still hold `old`,
        // else we roll back without auditing a stale change.
        if !accounts
            .get(account)
            .is_some_and(|stored| stored.deleted_at.is_none() && &stored.handle == old)
        {
            anyhow::bail!(
                "change_handle: account {} is not a live account still holding the expected \
                 handle; nothing changed (concurrent change or removal)",
                account
            );
        }

        // Uniqueness across every OTHER account, live or tombstoned.
        if accounts
            .iter()
            .any(|(id, stored)| id != account && &stored.handle == new)
        {
            return Err(anyhow::Error::new(HandleTaken));
        }

        let stored = accounts
            .get_mut(account)
            .expect("account presence checked by the precondition above");
        stored.handle = new.clone();
        stored.updated_at = at;
        drop(accounts);

        self.0
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned")
            .push(StoredHandleChange {
                account_id: account.clone(),
                old_handle: old.clone(),
                changed_at: at,
            });
        Ok(())
    }

    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()> {
        // Upsert into the (account, user) -> role map. A role grant never
        // clobbers a member's already-set alias.
        let UserAccount {
            user_id: user,
            account_id,
            role,
            alias: _,
        } = member;
        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        memberships.insert(
            (account_id.clone(), user.clone()),
            StoredMembership::listed(role.clone()),
        );
        Ok(())
    }

    async fn revoke_role(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()> {
        // A revoke is a departure, identical to `leave` at the store level.
        self.settle_member_departure(user, account);
        Ok(())
    }

    async fn leave(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()> {
        // Self-removal; the preconditions are the caller's.
        self.settle_member_departure(user, account);
        Ok(())
    }

    /// Insert the pending invitation unless one is already pending for the same
    /// `(account, invited_user)`, in which case this is a no-op. Returns the offer
    /// that now stands — the fresh one, or the pending one already on file.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<Invitation> {
        let mut invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        let already_pending = invitations.iter().find_map(|(id, stored)| {
            (stored.account == invitation.account
                && stored.invited_user == invitation.invited_user
                && stored.state == InvitationState::Pending)
                .then(|| rebuild_invitation(*id, stored))
        });
        if let Some(standing) = already_pending {
            // At most one pending offer per (account, user).
            return Ok(standing);
        }
        let issued = StoredInvitation {
            account: invitation.account.clone(),
            invited_user: invitation.invited_user.clone(),
            role: invitation.role.clone(),
            inviter: invitation.inviter.clone(),
            state: invitation.state,
            created_at: invitation.created_at,
            updated_at: invitation.updated_at,
        };
        let standing = rebuild_invitation(invitation.id, &issued);
        invitations.insert(invitation.id, issued);
        Ok(standing)
    }

    /// Flip a pending invitation to revoked and stamp `updated_at`. A non-pending
    /// or absent invitation is a no-op, not an error.
    async fn revoke_invitation(&mut self, id: &InvitationId) -> anyhow::Result<()> {
        let mut invitations = self
            .0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned");
        if let Some(stored) = invitations.get_mut(id)
            && stored.state == InvitationState::Pending
        {
            stored.state = InvitationState::Revoked;
            stored.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Flip the pending invitation to Accepted and seat the invited User; the
    /// state and the membership land together or not at all. The STORE's state is
    /// what is checked, so an offer accepted or revoked in the meantime seats
    /// nothing and errors. Seating an already-seated pair is a no-op, leaving the
    /// original membership and returning whatever role is actually persisted.
    async fn accept_invitation(
        &mut self,
        invitation: Invitation,
        listed_on_profile: bool,
    ) -> anyhow::Result<UserAccount> {
        {
            // Matching no pending offer means it was accepted or revoked since,
            // so seat no member.
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
                        invitation.id
                    ));
                }
            }
        }

        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");
        // `or_insert_with` writes only when the pair is absent, so a re-seat
        // leaves the existing role and listing choice untouched.
        let seated = StoredMembership {
            role: invitation.role.clone(),
            alias: None,
            listed_on_profile,
        };
        let stored = memberships
            .entry((invitation.account.clone(), invitation.invited_user.clone()))
            .or_insert(seated);

        Ok(UserAccount {
            account_id: invitation.account.clone(),
            user_id: invitation.invited_user.clone(),
            role: stored.role.clone(),
            alias: stored.alias.clone(),
        })
    }

    /// Transfer ownership: promote the incoming member to sole `Owner` and demote
    /// the outgoing one to `Admin`, under one lock so both writes land together.
    /// Errors rather than half-transferring if either membership has vanished.
    async fn transfer_ownership(
        &mut self,
        old_owner: &UserId,
        new_owner: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<()> {
        let mut memberships = self
            .0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned");

        // Backstop: the outgoing Owner must still be the Owner of this account.
        let outgoing_seat = (account.clone(), old_owner.clone());
        let incoming_seat = (account.clone(), new_owner.clone());

        if !matches!(
            memberships.get(&outgoing_seat).map(|m| &m.role),
            Some(Role::Owner)
        ) {
            return Err(anyhow::anyhow!(
                "user {} is not the Owner of account {}; ownership not transferred",
                old_owner.as_ref(),
                account
            ));
        }

        // Backstop: the incoming Owner must still be a member of this account.
        if !memberships.contains_key(&incoming_seat) {
            return Err(anyhow::anyhow!(
                "user {} is not a member of account {}; ownership not transferred",
                new_owner.as_ref(),
                account
            ));
        }

        // Only the roles swap; each member keeps their own listing choice.
        if let Some(outgoing) = memberships.get_mut(&outgoing_seat) {
            outgoing.role = Role::Admin;
        }
        if let Some(incoming) = memberships.get_mut(&incoming_seat) {
            incoming.role = Role::Owner;
        }
        Ok(())
    }

    /// Stamp `deleted_at` on the account, keeping the row so its handle stays
    /// reserved while reads treat it as absent. Memberships and invitations are
    /// left in place. Idempotent on an already-deleted or absent account.
    async fn soft_delete(&mut self, account: &AccountId) -> anyhow::Result<()> {
        let mut accounts = self
            .0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned");
        if let Some(stored) = accounts.get_mut(account)
            && stored.deleted_at.is_none()
        {
            let now = Utc::now();
            stored.deleted_at = Some(now);
            stored.updated_at = now;
        }
        Ok(())
    }

    /// Remove the account row — freeing its handle — with every membership,
    /// invitation and handle-change row, and sever its boards, columns and cards.
    /// The commissions those cards pointed at are untouched (they are User-owned),
    /// and so are view grants, which are no longer an account rail. Custody keys
    /// are not modeled here; removing an absent account is a no-op.
    async fn hard_delete(&mut self, account: &AccountId) -> anyhow::Result<()> {
        self.0
            .accounts
            .lock()
            .expect("MemBackend accounts mutex poisoned")
            .remove(account);

        self.0
            .memberships
            .lock()
            .expect("MemBackend memberships mutex poisoned")
            .retain(|(member_account, _), _| member_account != account);

        self.0
            .invitations
            .lock()
            .expect("MemBackend invitations mutex poisoned")
            .retain(|_, invitation| &invitation.account != account);

        // Drop the handle-change rows too, else the quarantine would keep citing
        // a change row for an account that no longer exists.
        self.0
            .handle_changes
            .lock()
            .expect("MemBackend handle_changes mutex poisoned")
            .retain(|change| &change.account_id != account);

        // Sever the boards, and with them every column and card — the
        // commissions they pointed at survive.
        let boards: Vec<WorkflowId> = {
            let mut workflows = self
                .0
                .workflows
                .lock()
                .expect("MemBackend workflows mutex poisoned");
            let boards = workflows
                .iter()
                .filter(|(_, stored)| stored.account_id == *account)
                .map(|(id, _)| *id)
                .collect();
            workflows.retain(|_, stored| stored.account_id != *account);
            boards
        };

        self.0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned")
            .retain(|_, stored| !boards.contains(&stored.workflow_id));

        Ok(())
    }
}

/// The read half of an account unit of work over the unit's staged snapshot, so
/// reads see writes issued through the same handle. There is no lock to take in
/// process, so `find_for_update` is `find`.
#[async_trait]
impl AccountReads for MemAccountWrites {
    async fn find(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        MemAccountStore(self.0.clone()).find(id).await
    }

    async fn find_for_update(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        MemAccountStore(self.0.clone()).find(id).await
    }

    async fn role_of(
        &mut self,
        user: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<Option<Role>> {
        MemAccountStore(self.0.clone()).role_of(user, account).await
    }
}

/// In-memory [`Database`] write factory: `begin` snapshots the shared domain
/// maps into a private staging backend, isolating the unit's writes until it
/// commits.
pub struct MemDatabase(MemBackend);

#[async_trait]
impl Database for MemDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        // `staged` is what the write views mutate; `base` is an independent copy
        // taken at the same instant, never touched, that `commit` diffs against.
        let staged = self.0.stage();
        let base = staged.stage();
        Ok(Box::new(MemUnitOfWork {
            shared: self.0.clone(),
            base,
            staged,
        }))
    }
}

/// In-memory [`UnitOfWork`] that models transactional rollback: it holds the
/// shared store, a pristine `base` snapshot, and the `staged` copy the write
/// views mutate. [`commit`](MemUnitOfWork::commit) merges the diff back onto
/// `shared`; dropping the handle uncommitted discards both copies, so
/// uncommitted writes are invisible to the shared read stores.
pub struct MemUnitOfWork {
    /// The real, shared store the unit commits back onto.
    shared: MemBackend,
    /// A pristine deep copy taken at `begin` and never mutated; `commit` diffs
    /// `staged` against it to find what this unit actually touched.
    base: MemBackend,
    /// A private deep copy the unit's writes land in, reaching `shared` only on
    /// `commit`. The profile-cache `Arc` is shared, never staged.
    staged: MemBackend,
}

#[async_trait]
impl UnitOfWork for MemUnitOfWork {
    /// The account repo over this unit's staged snapshot; reads see the unit's
    /// own uncommitted writes.
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        Box::new(MemAccountWrites(self.staged.clone()))
    }

    /// The commission repo over this unit's staged snapshot.
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        Box::new(MemCommissionWrites(self.staged.clone()))
    }

    /// The changelog append surface over this unit's staged snapshot, so an entry
    /// commits atomically with the writes staged beside it.
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        Box::new(MemChangelogWrites(self.staged.clone()))
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        Box::new(MemUserWrites(self.staged.clone()))
    }

    /// The actor-super-table write surface over this unit's staged snapshot; it
    /// carries no delete, since identity rows are immortal.
    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        Box::new(MemActorIdentityWrites(self.staged.clone()))
    }

    /// The workflow write surface over this unit's staged snapshot, so a card's
    /// move and the neighbours it displaces land together.
    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_> {
        Box::new(MemWorkflowWrites(self.staged.clone()))
    }

    /// The column write surface over this unit's staged snapshot.
    fn columns(&mut self) -> Box<dyn ColumnWrites + '_> {
        Box::new(MemColumnWrites(self.staged.clone()))
    }

    /// The character write surface over this unit's staged snapshot.
    fn characters(&mut self) -> Box<dyn CharacterWrites + '_> {
        Box::new(MemCharacterWrites(self.staged.clone()))
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        // Without this call `base`/`staged` are simply dropped, rolling back.
        self.shared.merge(&self.base, &self.staged);
        Ok(())
    }

    /// Drop `base` and `staged` together, discarding every write in the unit —
    /// the same outcome as dropping the handle, made explicit.
    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Rebuild an [`Account`] from its stored parts; `None` when the row is
/// soft-deleted, which every read treats as absent. The id is passed in because
/// it is the map's key.
fn rebuild_account(id: AccountId, stored: &StoredAccount) -> Option<Account> {
    if stored.deleted_at.is_some() {
        return None;
    }
    let account = Account {
        id,
        handle: stored.handle.clone(),
        name: stored.name.clone(),
        created_at: stored.created_at,
        updated_at: stored.updated_at,
        deleted_at: stored.deleted_at,
    };
    Some(account)
}

/// Rebuild an [`Invitation`] from its stored parts.
fn rebuild_invitation(id: InvitationId, stored: &StoredInvitation) -> Invitation {
    Invitation {
        id,
        account: stored.account.clone(),
        invited_user: stored.invited_user.clone(),
        role: stored.role.clone(),
        inviter: stored.inviter.clone(),
        state: stored.state,
        created_at: stored.created_at,
        updated_at: stored.updated_at,
    }
}

/// In-memory [`DidMinter`] fake: a deterministic, unique-per-call synthetic
/// `did:plc:mem<n>` from an internal counter, with no keypair, genesis operation
/// or directory write.
#[derive(Default)]
pub struct MemDidMinter {
    /// Monotonic counter feeding the next DID's suffix, starting at 0.
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
    /// Hand back the next synthetic DID (`did:plc:mem<n>`, zero-padded to six
    /// digits); never fails, and `handle` is ignored.
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        Ok(Did::from(format!("did:plc:mem{n:06}")))
    }

    /// Hand back the next synthetic DID, same counter as [`mint`](Self::mint);
    /// the fake records no alias for either, so there is nothing to omit.
    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        Ok(Did::from(format!("did:plc:mem{n:06}")))
    }

    /// No-op: the fake registers nothing, so there is nothing to tombstone.
    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
        Ok(())
    }

    /// No-op: the fake mints no real operation, so there is no `alsoKnownAs` to
    /// re-point.
    async fn update_handle(&self, _did: &Did, _handle: &Handle) -> anyhow::Result<()> {
        Ok(())
    }
}

/// In-memory [`KeyStore`] fake: custody keys in a process-local map,
/// UNENCRYPTED — safe only because they never leave memory and the fake mints no
/// real DID. The real at-rest encryption lives in the pg adapter.
#[derive(Clone, Default)]
pub struct MemKeyStore {
    /// DID string → its custody keys; clones share the state.
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
    /// Store `keys` under `did`. A DID mints once, so a second `put` for the same
    /// DID is rejected rather than overwriting custody keys.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()> {
        let mut map = self.keys.lock().unwrap();
        if map.contains_key(AsRef::<str>::as_ref(did)) {
            anyhow::bail!("custody keys already exist for {}", did);
        }
        map.insert(did.to_string(), keys.clone());
        Ok(())
    }

    /// Return the custody keys for `did`, or `None` if never stored.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>> {
        Ok(self
            .keys
            .lock()
            .unwrap()
            .get(AsRef::<str>::as_ref(did))
            .cloned())
    }
}

/// One appended operation as [`MemPlcOperationLog`] keeps it.
#[derive(Clone)]
struct MemPlcEntry {
    did: String,
    cid: String,
    op_type: String,
    prev: Option<String>,
    operation_json: String,
}

/// In-memory [`PlcOperationLog`] fake: appended operations in submission order,
/// in a process-local vec.
#[derive(Clone, Default)]
pub struct MemPlcOperationLog {
    /// Appended entries in order; clones share the state.
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
    /// Append the operation in submission order, mirroring pg's two integrity
    /// indexes: a duplicate `cid` is rejected, and so is a second non-genesis op
    /// chaining an already-used `prev` — the chain never forks.
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        if entries.iter().any(|entry| entry.cid == record.cid) {
            anyhow::bail!("plc operation {} already logged", record.cid);
        }
        if let Some(prev) = &record.prev
            && entries.iter().any(|entry| {
                entry.did == AsRef::<str>::as_ref(&record.did)
                    && entry.prev.as_deref() == Some(prev)
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
            .find(|entry| entry.did == AsRef::<str>::as_ref(did))
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
            .find(|entry| entry.did == AsRef::<str>::as_ref(did))
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
mod tests;
