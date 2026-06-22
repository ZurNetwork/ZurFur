//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates (`adapter-pg`, `adapter-mem`). This is the first one; as the
//! `domain` crate splits into per-domain crates, `UserRepo` moves with the
//! `User` entity into the `identity` namespace.

use async_trait::async_trait;

use crate::elements::{
    account::{Account, AccountId},
    did::Did,
    profile::Profile,
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
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

/// Zurfur's record of accounts and who owns them — an app-private store (see
/// DESIGN/"Domains and Applications"). An account and its founder's Owner
/// membership are minted together; persisting them is a single private-side
/// transaction, never a cross-store dual write (ZMVP-14, DESIGN/Account).
#[async_trait]
pub trait AccountRepo: Send + Sync {
    /// Persist a freshly founded account together with its Owner membership,
    /// atomically. Both rows live in the private store, so this is one unit of
    /// work. (ZMVP-14: "the creating User becomes Owner.")
    async fn create(&self, account: &Account, owner: &UserAccount) -> anyhow::Result<()>;

    /// Resolve an AccountId back to its Account, or None if no such account
    /// exists (or it has been soft-deleted).
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>>;

    /// The role a user holds in an account, or None if they are not a member.
    /// Lets callers verify membership/authority without loading every member.
    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>>;

    /// Set the role a user holds in an account, seating them if they aren't yet a
    /// member. On this platform granting a role *is* how a user joins an account
    /// (DESIGN/Roles), so this is an upsert: a brand-new member is inserted, an
    /// existing one's role is replaced. Idempotent — re-granting the held role is
    /// a no-op write. Authorization (who may grant what) is decided by the caller
    /// before this is reached; the store only persists the settled grant. Both
    /// rows live in the private store, so this is one private-side write, never a
    /// cross-store dual write (ZMVP-15, DESIGN/Roles).
    async fn grant_role(&self, member: &UserAccount) -> anyhow::Result<()>;
}

/// Mints a sovereign `did:plc` for a platform-custodied entity (an Account is
/// its own sovereign identity — see DESIGN/Account, DESIGN/"DID:PLC vs DID:Web").
/// Unlike a *visitor's* DID, which precedes us and is only ever recognized, an
/// account's DID is created on its behalf, server-side and invisibly.
///
/// FLOOR STUB (ZMVP-14): the live implementation returns a structurally-shaped
/// but synthetic `did:plc:` value. The real minter — keypair generation, PLC
/// genesis operation, signing, directory submission, PDS slot, key custody —
/// is deferred to its own infrastructure/security DD ("dress when The Who
/// closes"). Callers depend only on this port, so dressing it later is an
/// adapter swap, not a handler change.
#[async_trait]
pub trait DidMinter: Send + Sync {
    /// Mint a new account DID. Fallible because the real implementation performs
    /// a network write to the PLC directory; the stub never fails in practice.
    async fn mint(&self) -> anyhow::Result<Did>;
}
