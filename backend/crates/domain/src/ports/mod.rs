//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates (`adapter-pg`, `adapter-mem`). As the `domain` crate splits
//! into per-domain crates, each port moves with its entity (`UserStore`/
//! `UserWrites` into the `identity` namespace, …). Per-area ports live in
//! submodules ([`commission`], [`changelog`]) and are re-exported flat here, so
//! a new area adds a file rather than growing this one.

pub mod changelog;
pub mod commission;

pub use changelog::{ChangelogStore, ChangelogWrites};
pub use commission::{CommissionStore, CommissionWrites};

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use crate::datetime::DateTimeUtc;
use crate::elements::{
    account::{Account, AccountId},
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

/// The factory for a private-store [`UnitOfWork`] — the **only** way to reach a
/// write. It holds the connection pool and serves *reads* (via the per-aggregate
/// read stores), but the *write* methods live solely on the [`UnitOfWork`] handle
/// it vends, so a private-store write is unrepresentable without first opening a
/// transaction. This is "transactions as a capability" by construction: no bare
/// pool (an `Executor` that can issue any statement) is ever in scope at a write
/// site (DD "Transactions as a capability — a compile-enforced Unit of Work in
/// the private store", `24150017`).
///
/// Aggregate-neutral on purpose: `begin()` is **not** a method on any aggregate's
/// repo, because two per-aggregate transactions could not be made atomic
/// together. One `begin()` opens one transaction; the handler threads N writes
/// across aggregates through its view accessors and `commit()`s once.
#[async_trait]
pub trait Database: Send + Sync {
    /// Begin one private-store transaction; the returned handle owns it. The
    /// handler issues its writes through the handle's view accessors and then
    /// [`UnitOfWork::commit`]s exactly once; **forgetting to commit rolls back**
    /// (drop = rollback). This is strictly intra-Postgres — never a cross-store
    /// dual write (a PDS publish stays a separate retryable step).
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>>;
}

/// One open private-store transaction, owned by the handler. Aggregate writes are
/// reached as **views over this shared transaction** through the accessor methods
/// (`uow.accounts().create(...)`); every view borrows the one transaction, so all
/// writes in the unit land together on [`commit`](UnitOfWork::commit) or not at
/// all. The handle holds only the transaction — no pool — so nothing on this path
/// can skip the transaction.
#[async_trait]
pub trait UnitOfWork: Send {
    /// A view of the [`Account`] write surface over **this** transaction. The
    /// returned box borrows the handle, tying the view to the shared tx; drop it
    /// (end of statement) before calling another accessor or [`commit`](UnitOfWork::commit).
    fn accounts(&mut self) -> Box<dyn AccountWrites + '_>;

    fn commissions(&mut self) -> Box<dyn CommissionWrites + '_>;

    /// A view of the commission-changelog **append** surface over this
    /// transaction (ZMVP-87). On the Unit of Work — never pool-backed — because
    /// an entry must commit **atomically with the domain write it records**
    /// (Changelog DD `30408741` D4, no dual write): the emitter issues its
    /// domain write and its `append` through this same open unit.
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_>;

    /// A view of the [`User`] write surface (recognition) over this transaction.
    fn users(&mut self) -> Box<dyn UserWrites + '_>;

    /// Commit the unit, consuming the handle. Every write issued through the view
    /// accessors lands atomically. Not calling this — dropping the handle — rolls
    /// the whole unit back.
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;

    /// Abort the unit; awaited so the rollback is deterministic rather than relying on drop.
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;
}

/// Run `f` inside one private-store transaction. Opens a [`UnitOfWork`] via
/// [`Database::begin`], hands it to `f`, then **commits on `Ok`, rolls back on
/// `Err`** — the closure body *is* the transaction boundary, so a commit can never
/// be forgotten. Strictly intra-Postgres; never a cross-store dual write.
///
/// `f` is a plain closure that returns a boxed, `Send` future
/// (`|uow| Box::pin(async move { … })`) rather than an `async |uow| …` closure. An
/// `AsyncFnOnce(&mut dyn UnitOfWork)` bound would be more ergonomic, but an async
/// closure whose future borrows its `&mut` argument cannot satisfy the *higher-ranked*
/// `Send` bound Axum requires of a handler future (rust-lang/rust#100013 — "implementation
/// of `AsyncFnOnce` is not general enough"). Boxing the future — the same shape sqlx's
/// own transaction-closure API uses — sidesteps that limitation while keeping one call:
/// `commit`/`rollback` is still impossible to forget.
pub async fn transaction<T, F>(db: &dyn Database, f: F) -> anyhow::Result<T>
where
    F: for<'a> FnOnce(
        &'a mut Box<dyn UnitOfWork>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>,
{
    let mut uow = db.begin().await?;
    match f(&mut uow).await {
        Ok(value) => {
            uow.commit().await?;
            Ok(value)
        }
        Err(err) => {
            // The closure's error is the meaningful one (e.g. `HandleTaken` → 409);
            // a rollback failure must never replace it. The unit is abandoned either
            // way (an uncommitted transaction also rolls back on drop), so a rollback
            // error here is secondary and deliberately not surfaced over `err`.
            let _ = uow.rollback().await;
            Err(err)
        }
    }
}

/// The write surface of Zurfur's record of recognized visitors — reachable only
/// on an open [`UnitOfWork`]. Recognition is a private-store write, so it cannot
/// skip a transaction (see [`Database`]).
#[async_trait]
pub trait UserWrites: Send {
    /// Recognize a DID. The first call mints a User; every later call with the
    /// same DID returns that same User. One DID, one User, forever — idempotent,
    /// so callers needn't check existence first. (Criteria 1 & 2.)
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User>;
}

/// The read surface of Zurfur's record of recognized visitors. Identity precedes
/// us, so this port *recognizes* rather than registers (see ZMVP-9, DESIGN/User).
/// Reads are pool-backed and non-transactional — they pay no transaction tax;
/// recognition (the write) lives on [`UserWrites`].
#[async_trait]
pub trait UserStore: Send + Sync {
    /// Resolve a session's stored UserId back to its User, without touching the
    /// PDS. Returns None if no such User exists. (Criterion 3.)
    async fn find(&self, id: UserId) -> anyhow::Result<Option<User>>;

    /// Resolve a DID to its User *without minting one* — the read-only counterpart
    /// to [`UserWrites::provision`]. Returns None if no User has ever been
    /// recognized for that DID. Lets a caller act on an existing member by their
    /// public id (e.g. revoke a role) without the side effect of recognizing a
    /// brand-new visitor.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>>;
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
/// need the PDS awake (ZMVP-10 criterion 2). Both the read (`get`) and the
/// best-effort cache fill (`put`) are pool-backed and `&self`. The cache is a
/// **documented exception** to the compile-enforced Unit of Work (DD `24150017`):
/// a read-through cache fill carries no transactional invariant — it is a
/// single-statement, idempotent upsert issued on the GET read path and swallowed
/// on failure — so it does not belong on the write-only [`UnitOfWork`] handle, the
/// same reasoning that exempts `session_store` and `auth_store`. Freshness/TTL
/// policy lives in the implementation; a caller treats a miss — absent or stale —
/// as `None`.
#[async_trait]
pub trait ProfileCache: Send + Sync {
    /// Return the cached profile for a DID, or `None` on a miss — which the
    /// caller treats the same whether the entry is absent or judged stale. The
    /// `Result` is for store errors (e.g. the cache backend is down), not misses.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>>;

    /// Store (or refresh) a profile after a [`ProfileSource::fetch`], keyed by its
    /// DID. Idempotent: writing the same profile twice just refreshes the entry. A
    /// best-effort cache fill on the read path — not a domain write — so it is
    /// pool-backed and exempt from the Unit of Work (see the trait note).
    async fn put(&self, profile: &Profile) -> anyhow::Result<()>;
}

/// The **read** surface of Zurfur's record of accounts and who owns them — an
/// app-private store (see DESIGN/"Domains and Applications"). Reads are
/// pool-backed and non-transactional (no transaction tax); every *write* lives on
/// [`AccountWrites`], reachable only on an open [`UnitOfWork`] (ZMVP-14,
/// DESIGN/Account; DD `24150017`).
#[async_trait]
pub trait AccountStore: Send + Sync {
    /// Resolve an AccountId back to its Account, or None if no such account
    /// exists (or it has been soft-deleted).
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>>;

    /// The role a user holds in an account, or None if they are not a member.
    /// Lets callers verify membership/authority without loading every member.
    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>>;

    /// The pending invitation for `(account, invited_user)`, or `None` if there
    /// isn't one. Underpins the idempotent re-invite (a hit means "already
    /// invited", so ping rather than issue a duplicate). Only ever returns a
    /// pending offer — accepted/revoked invitations are history, not live offers.
    async fn find_pending_invitation(
        &self,
        account: AccountId,
        invited_user: UserId,
    ) -> anyhow::Result<Option<Invitation>>;

    /// Resolve an [`InvitationId`] to its [`Invitation`] in any state, or `None`.
    /// Lets the revoke path load the offer to check the inviter's authority and
    /// its current state before transitioning it.
    async fn find_invitation(&self, id: InvitationId) -> anyhow::Result<Option<Invitation>>;

    /// Resolve a live account's [`Handle`] to its sovereign [`Did`], or `None` if
    /// no live account holds it. Backs atproto handle resolution — the
    /// `/.well-known/atproto-did` endpoint for Zurfur-issued `*.zurfur.app` handles
    /// (ZMVP-44, DD/26607618) — and the founding-time duplicate-handle pre-check.
    /// Soft-deleted accounts don't match, mirroring [`find`](AccountStore::find).
    /// The `handle` is already normalized (it is a validated [`Handle`]), so this is
    /// an exact-match lookup, not a normalizing one.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>>;

    /// How many handle changes `account` has recorded on or after `since` — the count
    /// the caller weighs against the light anti-abuse rate limit before allowing
    /// another change (DD "Account Handle Change Flow" `27852802` §3). A pool-backed
    /// read over the `account_handle_changes` audit log; the limit and the window are
    /// the caller's policy (it passes `since = now − window`).
    async fn count_handle_changes_since(
        &self,
        account: AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64>;

    /// Whether `handle` is currently **quarantined to a different account** — vacated
    /// by some *other* account on or after `since` (the window floor `now − quarantine
    /// window` the caller computes) and so held reserved to whoever left it (DD
    /// `27852802` §4). `excluding` is the account asking, so it can *reclaim* its own
    /// just-vacated handle: a row whose `account_id` equals `excluding` never counts.
    /// Within the window a given `*.zurfur.app` handle maps to at most one prior holder,
    /// so a hit unambiguously means "reserved to someone else." Backs the availability
    /// check at both claim sites (founding and change). BYO handles are never
    /// quarantined, so callers only ask this for a Zurfur-namespace handle.
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool>;
}

/// The error a [`AccountWrites::create`] failure carries (as the source of its
/// `anyhow::Error`) when the account's handle collides with one already stored.
///
/// The `accounts` handle index is **global**, not scoped to live rows: a
/// soft-deleted (tombstoned) account still reserves its handle, and it is freed
/// only when the row is actually removed (hard delete) — DD `23003138` "Account
/// Deletion, Tombstoning & Handle Reuse". So a collision can be with a live *or* a
/// soft-deleted account.
///
/// Adapters return it so the founding handler can `downcast_ref` and answer `409`
/// rather than a generic `500`. The handler's `find_did_by_handle` pre-check is a
/// fast path for the common **live** collision; this is the authoritative backstop
/// for the two cases the pre-check cannot see — a **soft-deleted** reservation
/// (the pre-check filters those out) and the **concurrent-claim race** (two founds
/// pass the pre-check, one loses at the unique index).
#[derive(Debug)]
pub struct HandleTaken;

impl std::fmt::Display for HandleTaken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "account handle already taken")
    }
}

impl std::error::Error for HandleTaken {}

/// The **write** surface of Zurfur's record of accounts and memberships —
/// reachable only on an open [`UnitOfWork`] (`uow.accounts()`), so no private-store
/// account write can skip a transaction. An account and its founder's Owner
/// membership are minted together; persisting them is a single private-side
/// transaction, never a cross-store dual write (ZMVP-14, DESIGN/Account; DD
/// `24150017`).
#[async_trait]
pub trait AccountWrites: Send {
    /// Persist a freshly founded account together with its Owner membership,
    /// atomically. Both rows live in the private store, so this is one unit of
    /// work. (ZMVP-14: "the creating User becomes Owner.")
    ///
    /// A handle collision (the global unique handle index — live **or** tombstoned,
    /// DD `23003138`) fails with [`HandleTaken`] as the error source, so the caller
    /// can map it to a `409`; any other failure is an opaque store error.
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()>;

    /// Change an account's handle: repoint `accounts.handle` to `new` **and** append the
    /// change to the audit log — the record that both rate-limits future changes and
    /// quarantines the vacated `old` handle (DD "Account Handle Change Flow" `27852802`
    /// §3/§4) — atomically on the open transaction. The public half (re-pointing the DID
    /// document's `alsoKnownAs` via [`DidMinter::update_handle`]) is a **separate**
    /// retryable step the caller runs **first**, never inside this transaction (DD §7; no
    /// cross-store dual write). A collision with the global `accounts_handle_key` index —
    /// a handle held by another account, live **or** tombstoned (DD 23003138) — fails
    /// with [`HandleTaken`] so the caller can answer `409`, exactly as [`create`] does.
    /// `old` is an **optimistic-concurrency precondition**: the change applies only if the
    /// account is live and *still* holds `old` — a soft-deleted/vanished account, or one
    /// whose handle moved under a concurrent rename, fails and records **no** audit row,
    /// so the log can never capture a stale `old_handle` (which would leave the truly
    /// vacated handle un-quarantined). `at` is the change instant. Changing to the
    /// account's own current handle is a caller-side no-op rejected before this is
    /// reached. A private-side write, never a cross-store dual write.
    ///
    /// [`create`]: AccountWrites::create
    /// [`DidMinter::update_handle`]: DidMinter::update_handle
    async fn change_handle(
        &mut self,
        account: AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()>;

    /// Set the role a user holds in an account, seating them if they aren't yet a
    /// member. On this platform granting a role *is* how a user joins an account
    /// (DESIGN/Roles), so this is an upsert: a brand-new member is inserted, an
    /// existing one's role is replaced. Idempotent — re-granting the held role is
    /// a no-op write. Authorization (who may grant what) is decided by the caller
    /// before this is reached; the store only persists the settled grant. Both
    /// rows live in the private store, so this is one private-side write, never a
    /// cross-store dual write (ZMVP-15, DESIGN/Roles).
    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()>;

    /// Remove a user's membership in an account — an `Owner`/`Admin` removing someone
    /// (ZMVP-16). A member-departure event with the **same store effects as**
    /// [`leave`](AccountWrites::leave): in one transaction it re-homes the member's
    /// children to their parent (DESIGN/Roles rule 3), deletes the membership, and
    /// revokes the member's still-pending *issued* invitations (ZMVP-40 — so none can
    /// later seat a member under a non-member). Idempotent: removing a non-member is a
    /// no-op. Authorization (who may revoke whom) is the caller's concern, settled
    /// before this is reached. A private-side write, never a cross-store dual write.
    async fn revoke_role(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()>;

    /// A member **leaves** their own account on their own action (ZMVP-21). Unlike
    /// [`revoke_role`](AccountWrites::revoke_role) — which an `Owner`/`Admin` invokes on
    /// *someone else*, an authority action gated by rank — `leave` is **self-initiated**:
    /// it is the consent-symmetric counterpart to accepting an invitation (ZMVP-20)
    /// — joining took the user's yes, so does leaving — so it needs no rank check on
    /// the actor. In one transaction it re-homes the leaver's role-tree children to the
    /// leaver's own parent (DESIGN/Roles rule 3), deletes the membership, and revokes
    /// the leaver's still-pending *issued* invitations, so none can later seat a member
    /// under a non-member (DD "Invitation Validity & Issuer Departure", ZMVP-40). The
    /// caller settles the preconditions first — a non-member is turned away and the
    /// `Owner` cannot leave while still `Owner` — so this assumes a valid, non-`Owner`
    /// member; a vanished membership (a concurrent removal) is a no-op, not an error. A
    /// private-side write, never a cross-store dual write.
    async fn leave(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()>;

    /// Persist a freshly issued, pending [`Invitation`] (ZMVP-32 — the issuing
    /// half of invite-then-accept). At most one *pending* invitation may exist per
    /// (account, invited user): if one already does, this is a no-op rather than a
    /// second row — the store-level backstop for the idempotent re-invite the
    /// caller also guards by checking [`AccountStore::find_pending_invitation`]
    /// first. Authority (the inviter being Owner/Admin, the offered role below their
    /// rank) is the caller's check via `Role::can_grant`, settled before this is
    /// reached. A private-side write, never a cross-store dual write (DESIGN/Roles).
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<()>;

    /// Transition a pending invitation to revoked, so it can no longer be accepted
    /// (ZMVP-32). Idempotent on a non-pending or absent invitation — a no-op, not
    /// an error; the caller decides whether absence/already-revoked is a 404/409.
    /// *Who* may revoke (the issuing member) is the caller's authority check. A
    /// private-side write, never a cross-store dual write (DESIGN/Roles).
    async fn revoke_invitation(&mut self, id: InvitationId) -> anyhow::Result<()>;

    /// Accept a pending invitation: in ONE private-store transaction (the same unit
    /// of work as `create`, never a cross-store dual write) flip the invitation to
    /// Accepted AND seat the invited User as a member, with `parent = inviter`
    /// (DESIGN/Roles rule 4a — the first real write of `account_members.parent`) and
    /// the invitee's `listed_on_profile` choice (new column, default true). The
    /// implementation owns the guard: it flips only an offer that is *still* pending
    /// in the store, so a lost race (already accepted/revoked) seats no member — the
    /// guard is atomic with the seat, not a caller pre-check. Authority is the
    /// caller's, settled before this is reached (ZMVP-20).
    async fn accept_invitation(
        &mut self,
        invitation: Invitation,
        listed_on_profile: bool,
    ) -> anyhow::Result<UserAccount>;

    /// Transfer ownership of an account from its current Owner to another existing
    /// member (ZMVP-33; DESIGN/Roles rule 8), in ONE private-store transaction: the
    /// incoming member becomes the sole `Owner` with no parent (rule 5) and the
    /// outgoing Owner is demoted to `Admin`, re-homed under the new Owner. Ownership
    /// is singular, so this is its own seam — distinct from `grant_role` (which never
    /// grants Owner) and `leave`. Principals are addressed by id, like [`leave`] and
    /// [`revoke_role`]. Authority (the actor being the current Owner, the target being
    /// an existing member) is the caller's check, settled before this is reached; the
    /// implementation keeps a defensive backstop but does not re-authorize. A
    /// private-side write, never a cross-store dual write — the account's `did:plc` is
    /// stable, so no PLC write is involved.
    ///
    /// [`leave`]: AccountWrites::leave
    /// [`revoke_role`]: AccountWrites::revoke_role
    async fn transfer_ownership(
        &mut self,
        old_owner: UserId,
        new_owner: UserId,
        account: AccountId,
    ) -> anyhow::Result<()>;

    /// **Soft-delete** an account: mark it deactivated (`accounts.deleted_at = now`)
    /// without removing anything (ZMVP-34, DD `23003138`). The row stays, so the
    /// account's handle stays **reserved** (the global unique index spans soft-deleted
    /// rows) and its `did:plc` stays **live** — but reads treat it as absent
    /// ([`AccountStore::find`]/[`find_did_by_handle`](AccountStore::find_did_by_handle)
    /// already filter `deleted_at IS NULL`), so the account's public surface is hidden.
    /// Memberships and invitations are deliberately **kept**, so a later reactivation
    /// restores the account intact. In v1 (identity-only, no PDS — DD `26935298`)
    /// deactivation is purely this private-store state: the DID is untouched, so there
    /// is **no** atproto operation, and handle resolution stays truthful for the live
    /// DID. Owner-only, and gated on the account holding an **account-anchored fact**
    /// (per the Account Deletion DD `23003138` — **not** a commission, which is
    /// User-owned and survives account deletion, Ownership Separation DD `29130754`) —
    /// that policy is the caller's. Idempotent: re-soft-deleting a soft-deleted account
    /// is a no-op. A private-side write, never a cross-store dual write.
    async fn soft_delete(&mut self, account: AccountId) -> anyhow::Result<()>;

    /// **Hard-delete** an empty account: remove its `account_invitations`,
    /// `account_members`, and `accounts` rows in one unit of work (ZMVP-34, DD
    /// `23003138`). Removing the `accounts` row **frees the handle** for reuse — the
    /// global unique index no longer sees it — which is safe only because an account
    /// with no account-anchored fact carries no reputation. Only ever called for an
    /// account the caller has established holds **no account-anchored fact** (per the
    /// Account Deletion DD `23003138`); that gate is the caller's. The account's
    /// **positioning rails** — its commission placements and view grants (ZMVP-70) — are
    /// severed with it via `ON DELETE CASCADE`, but the placed **commissions survive
    /// untouched**: they are User-owned, never account facts (Ownership Separation DD
    /// `29130754`; ZMVP-57 AC1). The custody keys (`account_keys`) are left in place so
    /// the native ~72h `did:plc` tombstone recovery window can still reverse the
    /// deletion. **Tombstoning the DID is a separate retryable atproto step**, never
    /// part of this private transaction (no cross-store dual write — the mint path's
    /// mirror). Idempotent: hard-deleting an absent account is a no-op.
    async fn hard_delete(&mut self, account: AccountId) -> anyhow::Result<()>;
}

/// Why a [`PublicRecords`] operation failed.
///
/// The variants classify the **XRPC** outcome (transport reachability + the
/// response's status/error name), deliberately **not** the auth transport: a
/// later Bearer→OAuth switch (ZMVP-107) changes how the adapter authenticates but
/// not how a rejected write, a missing record, or an unreachable PDS surface —
/// so this mapping survives it. AC4 wants failures *distinguishable* (unreachable
/// vs rejected vs not-found), never a panic and never a silent success.
#[derive(Debug)]
pub enum PublicRecordsError {
    /// The PDS could not be reached at all (connection refused, DNS, timeout).
    /// A transient, retryable transport fault — nothing was written.
    Unreachable(anyhow::Error),
    /// The PDS answered but **refused** the operation: it carries the atproto
    /// error name (e.g. `InvalidRequest`, `AuthMissing`) and the HTTP status the
    /// server returned, so the caller can tell an authorization refusal from a
    /// bad request without string-matching a message.
    Rejected {
        /// The HTTP status the PDS returned.
        status: u16,
        /// The atproto error name (the `error` field of the XRPC error body).
        error: String,
        /// The optional human-readable detail (`message` field), if any.
        message: Option<String>,
    },
    /// The record failed structural validation before or at the write — a
    /// malformed record the repo will not accept.
    InvalidRecord(String),
    /// A [`get`](PublicRecords::get_record) (or a target of another op) named a
    /// record that does not exist.
    NotFound,
    /// Any other, unclassified failure (serialization, an unexpected response
    /// shape) — carried opaquely so nothing is swallowed.
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
/// read a record, and upload a blob, in the acting identity's atproto repo on its
/// PDS (DESIGN/"Domains and Applications", "Data Boundaries" `10354698`). The
/// mirror of [`Database`]'s private-store write surface, on the *public* side.
///
/// **Auth-agnostic by construction.** The port speaks [`Did`] + domain records
/// only; it never takes a credential per call. *How* the adapter authenticates as
/// the acting identity (a Bearer `CredentialSession` in ZMVP-105; a DPoP-bound
/// OAuth session later, ZMVP-107) is internal to the adapter, which is
/// **constructed with** its session. The PDS credential therefore lives inside
/// `adapter-atproto` and can never appear in a port signature or leak past the
/// crate boundary — the same containment plugins have (they never touch the PDS
/// credential; DD `24543244`).
///
/// **No cross-store transaction.** These are public-boundary writes, always their
/// own retryable step — never fused with a private-store [`UnitOfWork`] (the
/// mint/tombstone path's rule).
#[async_trait]
pub trait PublicRecords: Send + Sync {
    /// Create a new record in `repo`'s collection (the NSID is fixed by the
    /// record variant), letting the repo mint the rkey. Returns where it landed
    /// and the content hash of the written revision. The adapter can only write
    /// to the repo it is authenticated as; a `repo` it cannot act in is a
    /// [`PublicRecordsError::Rejected`].
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

    /// Delete the record at `uri`. Deleting an absent record is not an error
    /// (the repo treats it as a no-op), so a subsequent [`get_record`](PublicRecords::get_record) is the way
    /// to observe the deletion (it becomes [`PublicRecordsError::NotFound`]).
    async fn delete_record(&self, uri: &AtUri) -> Result<(), PublicRecordsError>;

    /// Read the record at `uri` back as a typed [`PublicRecord`]. A record that
    /// does not exist is [`PublicRecordsError::NotFound`].
    async fn get_record(&self, uri: &AtUri) -> Result<PublicRecord, PublicRecordsError>;

    /// Upload blob bytes to the acting identity's repo, returning the
    /// content-addressed [`BlobRef`] a record can then embed. Byte-identical
    /// uploads content-address to the same [`cid::Cid`].
    async fn upload_blob(
        &self,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<BlobRef, PublicRecordsError>;
}

/// Mints a sovereign `did:plc` for a platform-custodied entity (an Account is
/// its own sovereign identity — see DESIGN/Account, DESIGN/"DID:PLC vs DID:Web").
/// Unlike a *visitor's* DID, which precedes us and is only ever recognized, an
/// account's DID is created on its behalf, server-side and invisibly.
///
/// The live implementation (`adapter-atproto`'s `RealDidMinter`, ZMVP-49) is a
/// real minter: it generates per-account secp256k1 rotation keys, builds and
/// signs an **identity-only** PLC genesis operation (no PDS — the feed-generator
/// pattern, DD/26935298), derives the `did:plc` from its hash, persists the keys
/// envelope-encrypted via [`KeyStore`], and submits the operation to a PLC
/// directory. A synthetic floor stub (`StubDidMinter`) is kept for tests/dev.
/// Callers depend only on this port, so the choice is a composition-root swap.
#[async_trait]
pub trait DidMinter: Send + Sync {
    /// Mint a new account DID bound to `handle`: the handle becomes the genesis
    /// operation's initial `alsoKnownAs` (`at://<handle>`), the back-link half of
    /// atproto's bidirectional handle verification. Persisting the handle for
    /// resolution and maintaining it on later changes is a separate concern
    /// (ZMVP-44); this port only needs it to shape the operation it signs.
    ///
    /// Fallible: the real implementation generates keys, writes them to the
    /// [`KeyStore`], and submits to a directory — any of which can fail. The stub
    /// never fails in practice.
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did>;

    /// **Tombstone** a minted account DID (ZMVP-34 hard-delete, DD `23003138`). Signs
    /// a `plc_tombstone` operation with the account's custodied **operational**
    /// rotation key — chaining onto the DID's most recent operation (its `prev`, read
    /// from the [`PlcOperationLog`]) — records it in the log, and submits it to the
    /// directory. The DID's document is cleared and the identity permanently
    /// deactivated on the native ~72h PLC recovery window, during which a
    /// higher-authority rotation key can still reverse it (Zurfur retains the
    /// cold-recovery key).
    ///
    /// A **public-boundary** step, run **after** the private hard-delete has
    /// committed — never inside that transaction (no cross-store dual write, the mint
    /// path's mirror). Fallible and retryable: a directory failure leaves the freed
    /// handle and removed rows as they are; the tombstone is re-submittable. In v1 the
    /// directory is a gated no-op, so this signs and logs but registers nowhere. The
    /// stub/mem minters are no-ops.
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()>;

    /// **Re-point** a minted DID's `alsoKnownAs` to `handle` (ZMVP-50; consumed by
    /// the handle change, ZMVP-46, and usable to re-assert the current handle —
    /// "initial-maintain"). Signs a `plc_operation` with the account's custodied
    /// **operational** rotation key — the same shape as the genesis op, but with
    /// `alsoKnownAs` **REPLACED** by `["at://<handle>"]` (the old alias is dropped,
    /// DD `27852802` §5) and `prev` set to the DID's most recent operation's CID
    /// (read from the [`PlcOperationLog`], never fetched from the directory) —
    /// submits it to the directory, then records it in the log.
    ///
    /// A **public-boundary** step, always run as its own retryable unit — never
    /// inside a private-store transaction (no cross-store dual write; the caller
    /// owns any cross-store ordering, DD `27852802` §7). **Idempotent by
    /// content-address:** signing is deterministic (RFC 6979 + low-S), so an
    /// identical update — same `prev`, handle, and keys — has the same CID, and a
    /// replay dedups on the log's unique `cid`: "already logged" is success, not an
    /// error, which is what makes blind retries safe. A submission failure never
    /// advances the local chain, so a retry re-signs the *same* operation. Fails
    /// (retryably) if the DID has no custody keys or no logged operation to chain
    /// onto. In v1 the directory is a gated no-op, so this signs and logs but
    /// registers nowhere. The stub/mem minters are no-ops.
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()>;
}

/// Custody store for the private keys behind a minted `did:plc`. The pg adapter
/// **envelope-encrypts** every key under a root key before it touches disk, so a
/// database compromise alone never yields usable key material; a cloud-KMS
/// adapter (URGENT follow-up ZMVP-53, required before real accounts) drops in
/// behind this same port. The mem adapter keeps them in-process for tests.
///
/// This is a *private-store* port. Custody keys are the most sensitive rows
/// Zurfur holds: implementations must never persist them in the clear, and must
/// never log key material (the [`AccountKeys`] secrets are redacted in `Debug`
/// and zeroized on drop to help hold that line).
#[async_trait]
pub trait KeyStore: Send + Sync {
    /// Persist the custody keys for `did`, encrypted at rest. Called once, as part
    /// of minting, before the operation is submitted to the directory. Overwrites
    /// nothing by contract — one DID mints once.
    async fn put(&self, did: &Did, keys: &AccountKeys) -> anyhow::Result<()>;

    /// Load and decrypt the custody keys for `did`, or `None` if unknown. Future
    /// operations (rotation, `alsoKnownAs` updates — ZMVP-50/52) sign with the
    /// keys returned here; identity-only minting (ZMVP-49) only writes.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<AccountKeys>>;
}

/// Append-only log of the `did:plc` operations Zurfur has submitted for each minted
/// account identity — a *private-store* port. A DID is a chain of operations, and
/// every non-genesis operation references the CID of the DID's most recent operation
/// as its `prev`; since v1 does not fetch the chain back from the (gated) canonical
/// directory, Zurfur keeps its own record to chain the next operation and to audit
/// what it published (ZMVP-34 tombstone; reused by ZMVP-50/51, DD `23003138`).
///
/// The pg adapter persists these rows; the mem adapter keeps them in-process. The
/// records carry only public material (see [`PlcOperationRecord`]) — never a key.
#[async_trait]
pub trait PlcOperationLog: Send + Sync {
    /// Record a submitted operation. Appended once per operation, in submission order;
    /// the log is never mutated or pruned (a tombstone is just the last entry).
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()>;

    /// The CID of the DID's **most recent** logged operation — the `prev` a new
    /// operation must chain onto — or `None` if no operation is held for it.
    async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>>;

    /// The DID's **most recent** logged operation in full, or `None`. Unlike
    /// [`latest_cid`](PlcOperationLog::latest_cid), this returns the whole record so
    /// a handle update can carry the prior op's **public** document fields
    /// (`rotationKeys`, `verificationMethods`) forward from its `operation_json`
    /// verbatim — preserving the DID document while replacing only `alsoKnownAs` —
    /// **without decrypting any non-signing private key** (ZMVP-50 F2: the sibling
    /// `tombstone` needs only the operational key, and so should an update). The
    /// record carries public material only, never a key.
    async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>>;
}
