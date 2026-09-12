//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates. Per-area ports live in submodules ([`commission`],
//! [`changelog`], …) and are re-exported flat here, so a new area adds a file
//! rather than growing this one.

pub mod actor_identity;
pub mod changelog;
pub mod commission;
pub mod file;
pub mod workflow;

pub use actor_identity::{ActorIdentityStore, ActorIdentityWrites};
pub use changelog::{ChangelogStore, ChangelogWrites};
pub use commission::{
    CommissionReads, CommissionRepo, CommissionStore, CommissionWrites, ElementNotFound,
    UnknownSurface, UnknownTab,
};
pub use file::FileStore;
pub use workflow::{ColumnStore, ColumnWrites, WorkflowStore, WorkflowWrites};

use std::future::Future;

use async_trait::async_trait;

use crate::datetime::DateTimeUtc;
use crate::elements::{
    account::{Account, AccountId, AccountMembership, ListingScope},
    account_keys::AccountKeys,
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId},
    plc_operation::PlcOperationRecord,
    profile::Profile,
    public_record::{AtUri, BlobRef, PublicRecord, RecordRef},
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
};

/// The factory for a private-store [`UnitOfWork`] — the only way to reach a
/// private-store write, which is therefore unrepresentable without first opening
/// a transaction. Aggregate-neutral: one `begin()` opens one transaction for
/// writes across any number of aggregates. (DD 24150017)
#[async_trait]
pub trait Database: Send + Sync {
    /// Begin one private-store transaction; the returned handle owns it.
    /// Intra-Postgres only — never a cross-store dual write.
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>>;
}

/// One open private-store transaction. Aggregate writes are reached as views over
/// it through the accessors (`uow.accounts().create(…)`), so every write in the
/// unit lands on [`commit`](UnitOfWork::commit) or not at all. The handle holds
/// no pool, so nothing on this path can skip the transaction.
#[async_trait]
pub trait UnitOfWork: Send {
    /// The [`Account`] repo over this transaction ([`AccountRepo`]). The box
    /// borrows the handle; drop it before calling another accessor.
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_>;

    /// The commission repo over this transaction, reads and writes
    /// ([`CommissionRepo`]).
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_>;

    /// The commission-changelog append surface over this transaction, so an entry
    /// commits atomically with the domain write it records. (DD 59310081)
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_>;

    /// The [`User`] write surface (recognition) over this transaction.
    fn users(&mut self) -> Box<dyn UserWrites + '_>;

    /// The actor-super-table write surface over this transaction. It carries no
    /// delete — identity rows are immortal. (DD 34013187)
    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_>;

    /// The workflow write surface over this transaction; a board mutation is
    /// rarely one row.
    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_>;

    /// The column write surface over this transaction. Column *order* is a
    /// workflow write, not a column one.
    fn columns(&mut self) -> Box<dyn ColumnWrites + '_>;

    /// Commit the unit, consuming the handle; every write lands atomically.
    /// Dropping the handle instead rolls the whole unit back.
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;

    /// Abort the unit explicitly. Dropping the handle rolls back just the same;
    /// this exists for the legacy `transaction()` wrapper.
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;
}

/// The bound a transaction body closure must satisfy: callable once with a
/// borrowed [`UnitOfWork`] for any lifetime `'a`, yielding a `Send` future for
/// that same `'a`. Routed through plain [`FnOnce`] rather than `AsyncFnOnce`,
/// which cannot currently be proven at that higher rank (rust-lang/rust#110338),
/// so call sites need no `Box::pin`.
pub trait UnitOfWorkFn<'a, T>: FnOnce(&'a mut dyn UnitOfWork) -> Self::Fut {
    /// The future `Self` returns when called — named so a `for<'a>` bound can
    /// require it `Send + 'a` without knowing the closure's concrete type.
    type Fut: Future<Output = anyhow::Result<T>> + Send + 'a;
}

impl<'a, T, F, Fut> UnitOfWorkFn<'a, T> for F
where
    F: FnOnce(&'a mut dyn UnitOfWork) -> Fut,
    Fut: Future<Output = anyhow::Result<T>> + Send + 'a,
{
    type Fut = Fut;
}

/// The write surface of Zurfur's record of recognized visitors — reachable only
/// on an open [`UnitOfWork`].
#[async_trait]
pub trait UserWrites: Send {
    /// Recognize a DID: the first call mints a User, every later call returns
    /// that same User. One DID, one User, forever — idempotent.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User>;
}

/// The read surface of Zurfur's record of recognized visitors — pool-backed and
/// non-transactional; recognition (the write) lives on [`UserWrites`].
#[async_trait]
pub trait UserStore: Send + Sync {
    /// Resolve a stored UserId back to its User, or `None` if no such User
    /// exists. Never touches the PDS.
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>>;

    /// Resolve a DID to its User *without minting one*, or `None` if no User has
    /// ever been recognized for it.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>>;
}

/// Authenticates a visitor against their PDS, yielding the DID they already own.
/// The two methods mirror the OAuth handshake and speak only a handle, an opaque
/// redirect URL and a [`Did`], so the protocol library stays inside its adapter.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Begin sign-in for a handle; returns the PDS authorization URL to redirect
    /// the visitor to.
    async fn start(&self, handle: &str) -> anyhow::Result<String>;

    /// Complete the callback the PDS redirected back with, returning the
    /// authenticated visitor's DID.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
    ) -> anyhow::Result<Did>;
}

/// Reads a visitor's public profile from its source of truth, the user's PDS —
/// a public-boundary read: async, fallible, and possibly lagging.
#[async_trait]
pub trait ProfileSource: Send + Sync {
    /// Fetch the profile for a DID. The handle always resolves; `display_name`
    /// and `avatar_url` may be absent. Errors when the PDS is unreachable.
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile>;
}

/// A private-side read-through cache of public profiles, so repeat views don't
/// need the PDS awake. Pool-backed and `&self` — a documented exception to the
/// Unit of Work, since a cache fill carries no transactional invariant
/// (DD 24150017). Freshness policy lives in the implementation.
#[async_trait]
pub trait ProfileCache: Send + Sync {
    /// The cached profile for a DID, or `None` on a miss — absent and stale alike.
    /// The `Result` is for store errors, not misses.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>>;

    /// Store or refresh a profile, keyed by its DID. Idempotent; a best-effort
    /// fill on the read path, not a domain write.
    async fn put(&self, profile: &Profile) -> anyhow::Result<()>;
}

/// The **read** surface of Zurfur's record of accounts and who owns them —
/// pool-backed and non-transactional; every write lives on [`AccountWrites`].
#[async_trait]
pub trait AccountStore: Send + Sync {
    /// The live account `id` names, or `None` if absent or soft-deleted.
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// The role a user holds in an account, or `None` if they are not a member.
    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>>;

    /// The pending invitation for `(account, invited_user)`, or `None`. Only ever
    /// a pending offer; accepted/revoked ones are history. Underpins the
    /// idempotent re-invite.
    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>>;

    /// Resolve an [`InvitationId`] to its [`Invitation`] in any state, or `None`.
    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>>;

    /// The sovereign [`Did`] of the live account holding `handle`, or `None`.
    /// Exact-match; soft-deleted accounts never match. Backs atproto handle
    /// resolution and the founding-time duplicate check. (DD 26607618)
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>>;

    /// How many handle changes `account` has recorded on or after `since`. The
    /// rate limit and its window are the caller's policy. (DD 27852802)
    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64>;

    /// Whether `handle` was vacated by some account other than `excluding` on or
    /// after `since`, and so is still quarantined to it. `excluding` lets an
    /// account reclaim its own just-vacated handle. (DD 27852802)
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool>;

    /// Every live account `user` holds a role in, each paired with that role.
    /// Soft-deleted accounts excluded; ordered by [`AccountId`], unpaginated.
    ///
    /// `scope` is required so it cannot be forgotten: rendering one user's
    /// memberships to another must pass
    /// [`PublicProfile`](ListingScope::PublicProfile), which applies the
    /// `listed_on_profile` privacy valve.
    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>>;
}

/// Error source of an account write whose handle collides with one already
/// stored — live **or** soft-deleted, since the handle index spans both
/// (DD 23003138). Adapters return it so routes can `downcast_ref` and answer
/// `409` rather than a generic `500`.
#[derive(Debug)]
pub struct HandleTaken;

impl std::fmt::Display for HandleTaken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "account handle already taken")
    }
}

impl std::error::Error for HandleTaken {}

/// Marker error: the supplied DID is already interned as a different actor kind.
/// One DID = one actor (DD 34013187); routes downcast this to a
/// `409 did_belongs_to_another_actor`.
#[derive(Debug)]
pub struct DidBelongsToAnotherActor {
    /// The kind the DID is already interned as.
    pub existing_kind: String,
}

impl std::fmt::Display for DidBelongsToAnotherActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the DID already belongs to another actor (kind '{}')",
            self.existing_kind
        )
    }
}

impl std::error::Error for DidBelongsToAnotherActor {}

/// The **write** surface of Zurfur's record of accounts and memberships —
/// reachable only on an open [`UnitOfWork`] (`uow.accounts()`). Authorization is
/// always the caller's, settled before any of these is reached.
#[async_trait]
pub trait AccountWrites: Send {
    /// Persist a freshly founded account together with its founder's Owner
    /// membership, atomically. A handle collision fails with [`HandleTaken`] as
    /// the error source; anything else is an opaque store error.
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()>;

    /// Repoint `accounts.handle` to `new` and append the audit-log row that
    /// rate-limits future changes and quarantines `old`, atomically at `at`.
    /// `old` is an optimistic-concurrency precondition: a soft-deleted account, or
    /// one whose handle moved under a concurrent rename, fails and writes no audit
    /// row. A collision fails with [`HandleTaken`]. The public half — the DID
    /// document's `alsoKnownAs` — is a separate retryable step the caller runs
    /// first, never inside this transaction. (DD 27852802)
    async fn change_handle(
        &mut self,
        account: &AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()>;

    /// Set the role a user holds in an account, seating them if they aren't yet a
    /// member — an upsert, since granting a role *is* how a user joins. Idempotent.
    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()>;

    /// Remove a user's membership: re-homes their role-tree children to their own
    /// parent, deletes the membership, and revokes the invitations they had issued,
    /// in one transaction. Idempotent on a non-member. (DD 24182820)
    async fn revoke_role(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()>;

    /// A member leaves their own account: the same store effects as
    /// [`revoke_role`](AccountWrites::revoke_role), self-initiated so no rank check
    /// applies. Assumes a valid non-`Owner` member; a vanished membership is a
    /// no-op, not an error.
    async fn leave(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()>;

    /// Persist a freshly issued, pending [`Invitation`]. At most one pending
    /// invitation may exist per (account, invited user); a duplicate is a no-op
    /// rather than a second row.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<Invitation>;

    /// Transition a pending invitation to revoked. Idempotent on a non-pending or
    /// absent invitation — a no-op, not an error.
    async fn revoke_invitation(&mut self, id: &InvitationId) -> anyhow::Result<()>;

    /// Accept a pending invitation: in one transaction flip it to Accepted and seat
    /// the invited User with `parent = inviter` and their `listed_on_profile`
    /// choice. The guard is atomic with the seat — a lost race seats no member.
    async fn accept_invitation(
        &mut self,
        invitation: Invitation,
        listed_on_profile: bool,
    ) -> anyhow::Result<UserAccount>;

    /// Transfer ownership to another existing member, in one transaction: the
    /// incoming member becomes the sole `Owner` with no parent, the outgoing Owner
    /// is demoted to `Admin` under them. Its own seam because ownership is
    /// singular — `grant_role` never grants Owner.
    async fn transfer_ownership(
        &mut self,
        old_owner: &UserId,
        new_owner: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<()>;

    /// Soft-delete an account: stamp `deleted_at`, removing nothing. The handle
    /// stays reserved and the `did:plc` stays live, but reads treat the account as
    /// absent; memberships and invitations are kept so reactivation restores it
    /// intact. Idempotent. (DD 23003138)
    async fn soft_delete(&mut self, account: &AccountId) -> anyhow::Result<()>;

    /// Hard-delete an empty account: remove its invitation, membership and account
    /// rows in one unit of work, freeing the handle. Its boards cascade away; the
    /// commissions on them survive (User-owned) and the custody keys are left in
    /// place. Tombstoning the DID is a separate retryable step. Idempotent.
    /// (DD 23003138)
    async fn hard_delete(&mut self, account: &AccountId) -> anyhow::Result<()>;
}

/// The **read** side of an account unit of work — [`AccountStore`]'s lookups
/// on the unit's own connection (sees the unit's writes, may hold row locks).
#[async_trait]
pub trait AccountReads: Send {
    /// The live account `id` names, or `None`; as [`AccountStore::find`].
    async fn find(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// [`find`](Self::find), with the row locked (`FOR NO KEY UPDATE`) until commit.
    async fn find_for_update(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// `user`'s role in `account`, or `None`; as [`AccountStore::role_of`].
    async fn role_of(&mut self, user: &UserId, account: &AccountId)
    -> anyhow::Result<Option<Role>>;
}

/// An account unit of work: [`AccountReads`] + [`AccountWrites`] on one
/// connection. Vended by [`UnitOfWork::accounts`].
pub trait AccountRepo: AccountReads + AccountWrites {}

impl<T: AccountReads + AccountWrites> AccountRepo for T {}

/// Why a [`PublicRecords`] operation failed — the XRPC outcome, classified so a
/// caller can tell unreachable from rejected from not-found.
#[derive(Debug)]
pub enum PublicRecordsError {
    /// The PDS could not be reached at all. A transient, retryable transport
    /// fault — nothing was written.
    Unreachable(anyhow::Error),
    /// The PDS answered but refused the operation, carrying the atproto error
    /// name and HTTP status.
    Rejected {
        /// The HTTP status the PDS returned.
        status: u16,
        /// The atproto error name (the `error` field of the XRPC error body).
        error: String,
        /// The optional human-readable detail (`message` field), if any.
        message: Option<String>,
    },
    /// The record failed structural validation before or at the write.
    InvalidRecord(String),
    /// The operation named a record that does not exist.
    NotFound,
    /// Any other, unclassified failure, carried opaquely so nothing is swallowed.
    Unexpected(anyhow::Error),
}

impl std::fmt::Display for PublicRecordsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublicRecordsError::Unreachable(e) => write!(f, "PDS unreachable: {e}"),
            PublicRecordsError::Rejected {
                status,
                error,
                message,
            } => match message {
                Some(m) => write!(f, "PDS rejected ({status} {error}): {m}"),
                None => write!(f, "PDS rejected ({status} {error})"),
            },
            PublicRecordsError::InvalidRecord(why) => write!(f, "invalid record: {why}"),
            PublicRecordsError::NotFound => write!(f, "record not found"),
            PublicRecordsError::Unexpected(e) => write!(f, "unexpected public-records error: {e}"),
        }
    }
}

impl std::error::Error for PublicRecordsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PublicRecordsError::Unreachable(e) | PublicRecordsError::Unexpected(e) => {
                Some(e.as_ref())
            }
            _ => None,
        }
    }
}

/// The **write** surface of the public data boundary: create / put / delete /
/// read a record, and upload a blob, in the acting identity's atproto repo.
/// Auth-agnostic by construction — it speaks [`Did`] and domain records only,
/// never a credential, so the PDS credential stays inside its adapter. Every call
/// is its own retryable step, never fused with a private-store [`UnitOfWork`].
#[async_trait]
pub trait PublicRecords: Send + Sync {
    /// Create a new record in `repo`'s collection (the NSID follows the record
    /// variant), letting the repo mint the rkey. Returns where it landed. A `repo`
    /// the adapter cannot act in is a [`PublicRecordsError::Rejected`].
    async fn create_record(
        &self,
        repo: &Did,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError>;

    /// Upsert the record at `uri` (create-or-overwrite at that exact key).
    /// Returns the new [`RecordRef`]. Idempotent for identical content.
    async fn put_record(
        &self,
        uri: &AtUri,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError>;

    /// Delete the record at `uri`. Deleting an absent record is a no-op, not an
    /// error.
    async fn delete_record(&self, uri: &AtUri) -> Result<(), PublicRecordsError>;

    /// Read the record at `uri` back as a typed [`PublicRecord`], or
    /// [`PublicRecordsError::NotFound`].
    async fn get_record(&self, uri: &AtUri) -> Result<PublicRecord, PublicRecordsError>;

    /// Upload blob bytes to the acting identity's repo, returning the
    /// content-addressed [`BlobRef`] a record can embed. Byte-identical uploads
    /// address to the same CID.
    async fn upload_blob(
        &self,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<BlobRef, PublicRecordsError>;
}

/// Mints a sovereign `did:plc` for a platform-custodied entity, server-side —
/// unlike a visitor's DID, which precedes us and is only recognized. The live
/// minter signs an identity-only PLC genesis operation and persists the keys via
/// [`KeyStore`]; a stub floor is kept for tests. (DD 26935298)
#[async_trait]
pub trait DidMinter: Send + Sync {
    /// Mint a new account DID whose genesis `alsoKnownAs` is `at://<handle>`.
    /// Fallible: key generation, the [`KeyStore`] write, and the directory
    /// submission can each fail.
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did>;

    /// Sign, log and submit `plc_tombstone` for `did`, chained on its latest
    /// logged operation. A public-boundary step run after the private delete has
    /// committed, never inside that transaction; retryable. (DD 23003138)
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()>;

    /// Re-point `did`'s `alsoKnownAs` to `handle`, replacing the old alias, chained
    /// on its latest logged operation. Its own retryable step, never inside a
    /// private-store transaction; idempotent by content-address, so a replay dedups
    /// on the log's unique `cid`. Fails if the DID has no custody keys or no
    /// operation to chain onto. (DD 27852802)
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()>;
}

/// The public-boundary lifecycle operations an already-minted DID runs for
/// itself; call sites go through [`Did`], never this port directly. Every call is
/// its own retryable step, reached only after the owning private-store
/// transaction has committed. Interface only for now — no adapter implements it.
#[async_trait]
pub trait DidOperations: Send + Sync {
    /// Sign and submit `plc_tombstone` for `did`, chained on its latest logged
    /// operation. Re-submittable; a failure leaves prior state intact.
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()>;

    /// Sign and submit the `alsoKnownAs` replacement pointing `did` at
    /// `handle`. Idempotent by content-address; re-submittable.
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()>;
}

/// Custody store for the private keys behind a minted `did:plc` — a private-store
/// port. Implementations must never persist key material in the clear and must
/// never log it ([`AccountKeys`] redacts its secrets in `Debug` and zeroizes them
/// on drop).
#[async_trait]
pub trait KeyStore: Send + Sync {
    /// Persist the custody keys for `did`, encrypted at rest. Called once during
    /// minting; overwrites nothing by contract — one DID mints once.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()>;

    /// Load and decrypt the custody keys for `did`, or `None` if unknown.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>>;
}

/// Append-only log of the `did:plc` operations Zurfur has submitted — a
/// private-store port. Every non-genesis operation chains onto the CID of the
/// DID's most recent one, and v1 never refetches the chain from the directory, so
/// this log is what the next operation chains onto. Records carry public material
/// only, never a key.
#[async_trait]
pub trait PlcOperationLog: Send + Sync {
    /// Record a submitted operation, in submission order. The log is never mutated
    /// or pruned; a tombstone is just the last entry.
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()>;

    /// The CID of the DID's most recent logged operation — the `prev` a new one
    /// must chain onto — or `None` if none is held.
    async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>>;

    /// The DID's most recent logged operation in full, or `None`, so a handle
    /// update can carry the prior op's public document fields forward without
    /// decrypting any key.
    async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>>;
}
