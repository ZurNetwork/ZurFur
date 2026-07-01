//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates (`adapter-pg`, `adapter-mem`). This is the first one; as the
//! `domain` crate splits into per-domain crates, `UserStore`/`UserWrites` move with the
//! `User` entity into the `identity` namespace.

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use crate::elements::{
    account::{Account, AccountId},
    account_keys::AccountKeys,
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId},
    profile::Profile,
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
