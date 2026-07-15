//! The [`ActorIdentity`] — a row in the actor super-table (ZMVP-122, DD `34013187`).
//!
//! One row per actor the Index has ever seen (User, Account, Character; a seated
//! Golem is a User), the single table every actor reference FKs into. Built
//! **incrementally**: this slice is existence only — the identity is just its
//! app-private id. `kind`, the optional `did`, `handle`, and liveness `state`
//! arrive in later slices, each with its own tests.
//!
//! Two invariants are born here, ahead of the columns:
//! - **Rows are immortal.** There is no delete anywhere on the port
//!   ([`crate::ports::ActorIdentityWrites`]); liveness will be a *state* on the
//!   row, never a removal, so an FK into `actor_identity` can never break.
//! - **Actor-ness is anchored on the internal id, not a DID.** Characters are
//!   actors and carry no DID (Engineer ruling 2026-07-14), so the DID — when it
//!   arrives — is an optional external alias, never the essence.
//!
//! Slice 2 adds [`ActorKind`] — the closed vocabulary (`user | account |
//! character`) and, with `UNIQUE (id, kind)` in the schema, the anchor every
//! kind-checked reference site's composite FK targets (DD decisions 2 and 4).
//!
//! Slice 3 adds the **optional** [`Did`] — an external alias, never the
//! essence: `UNIQUE` where present (one DID = one actor, ever, DB-enforced),
//! absent on DID-less actors (Characters). DID-bearing actors are created by
//! [`crate::ports::ActorIdentityWrites::intern`] — race-safe and idempotent by
//! DID; DID-less ones by [`crate::ports::ActorIdentityWrites::create`].
//!
//! Slice 4 adds [`ActorState`] — liveness as a state on the row, the split
//! that replaces deletion (DD decisions 3/5): every row is born
//! [`ActorState::Active`]; `pulled`/`tombstoned` are recorded endings, never
//! removals. The transitions and the read-path predicate are ZMVP-125's.
//!
//! Slice 5 adds the cached handle — a refreshable **display cache** of the
//! actor's atproto handle, deliberately a plain string: external handles are
//! foreign data and never pass through Zurfur's claim-validation
//! ([`crate::elements::handle::Handle`] stays the claim gate's type).
//!
//! Slice 7 adds `first_seen` — when the Index first saw the actor, stamped at
//! create/intern and immutable thereafter (re-interning a DID keeps the
//! original stamp). Injected, never `now()`-defaulted, per house convention.

use std::ops::Deref;

use crate::datetime::DateTimeUtc;
use crate::elements::did::Did;

/// The app-private, stable handle for an [`ActorIdentity`] row.
///
/// A UUIDv7 wrapped for type safety, so an actor-identity id can't be passed
/// where some other id is wanted. This is the anchor every kind-checked actor
/// reference will FK to (DD `34013187` decision 4). Deref exposes the inner UUID.
///
/// References: [`new`](ActorIdentityId::new), [`ActorIdentity::mint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorIdentityId(uuid::Uuid);

impl ActorIdentityId {
    /// Rebuilds an id from its stored UUID — e.g. a row read back from Postgres.
    /// Minting a *fresh* id happens in [`ActorIdentity::mint`], not here.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for ActorIdentityId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// What kind of actor an identity row is — the closed vocabulary of DD
/// `34013187` decision 2. A seated Golem acts as a User, so there is no `golem`
/// variant and no occupant union. The unknown-kind representation for bare
/// network DIDs is deliberately **not** modelled yet (ZMVP-126 decides it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    User,
    Account,
    Character,
}

impl ActorKind {
    /// The stored spelling — exactly the values the schema's `CHECK` admits.
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorKind::User => "user",
            ActorKind::Account => "account",
            ActorKind::Character => "character",
        }
    }
}

/// The error a stored string that names no [`ActorKind`] parses to.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownActorKind(pub String);

impl std::fmt::Display for UnknownActorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown actor kind: {}", self.0)
    }
}

impl std::error::Error for UnknownActorKind {}

impl TryFrom<&str> for ActorKind {
    type Error = UnknownActorKind;

    /// Parse the stored spelling back. The schema's `CHECK` admits only the
    /// three variants, so an error here means a corrupted row, not user input.
    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "user" => Ok(ActorKind::User),
            "account" => Ok(ActorKind::Account),
            "character" => Ok(ActorKind::Character),
            other => Err(UnknownActorKind(other.to_string())),
        }
    }
}

/// An actor identity's liveness — a *state* on the immortal row, never a
/// removal (DD `34013187` decisions 3/5). Identity is permanent and
/// FK-enforced; liveness is soft and consulted per-read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorState {
    /// The normal case: the actor is live.
    Active,
    /// The DID/PDS stopped resolving — the reference is kept, the content is
    /// absent (the Data Boundaries `Pulled` semantics).
    Pulled,
    /// Deleted per the tombstone ruling: the identity is anonymized, the
    /// facts that reference it stay.
    Tombstoned,
}

impl ActorState {
    /// The stored spelling — exactly the values the schema's `CHECK` admits.
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorState::Active => "active",
            ActorState::Pulled => "pulled",
            ActorState::Tombstoned => "tombstoned",
        }
    }
}

/// The error a stored string that names no [`ActorState`] parses to.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownActorState(pub String);

impl std::fmt::Display for UnknownActorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown actor state: {}", self.0)
    }
}

impl std::error::Error for UnknownActorState {}

impl TryFrom<&str> for ActorState {
    type Error = UnknownActorState;

    /// Parse the stored spelling back. The schema's `CHECK` admits only the
    /// three variants, so an error here means a corrupted row, not user input.
    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "active" => Ok(ActorState::Active),
            "pulled" => Ok(ActorState::Pulled),
            "tombstoned" => Ok(ActorState::Tombstoned),
            other => Err(UnknownActorState(other.to_string())),
        }
    }
}

/// One actor's row in the super-table: its id, what kind of actor it is, its
/// optional [`Did`], and its liveness [`ActorState`]. The cached handle lands
/// in a later slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorIdentity {
    pub id: ActorIdentityId,
    pub kind: ActorKind,
    /// The actor's DID, when it has one. `None` is a designed state, not a
    /// gap: Characters are actors and carry no DID (Engineer ruling
    /// 2026-07-14) — actor-ness is anchored on [`ActorIdentityId`].
    pub did: Option<Did>,
    /// Liveness. Every row is born [`ActorState::Active`]; the transitions
    /// (and the read predicate that ghosts non-active actors) are ZMVP-125.
    pub state: ActorState,
    /// A refreshable display **cache** of the actor's atproto handle — foreign
    /// data, so a plain string, never the claim-validated
    /// [`Handle`](crate::elements::handle::Handle). `None` = nothing cached
    /// (DID-less actors, or not fetched yet). Rows are born uncached; the
    /// cache fills via [`crate::ports::ActorIdentityWrites::cache_handle`].
    pub handle: Option<String>,
    /// When the Index first saw this actor. An explicit domain fact, injected
    /// (tests and import flows stay deterministic) and immutable — re-seeing
    /// an actor never restamps it.
    pub first_seen: DateTimeUtc,
}

impl ActorIdentity {
    /// Mint a brand-new **DID-less** actor identity of `kind` with a fresh
    /// UUIDv7 key, first seen `now`. Any kind mints — the invariant is
    /// `did: None`, not the kind (in the domain, Characters are the actors
    /// born DID-less, DD `34013187`).
    ///
    /// Pure: this only builds the value — persisting it is
    /// [`crate::ports::ActorIdentityWrites::create`]'s job. Each call mints a
    /// distinct identity. DID-bearing actors go through
    /// [`crate::ports::ActorIdentityWrites::intern`] instead, which owns the
    /// race-safe one-DID-one-actor upsert. `now` is injected so tests and
    /// import flows stay deterministic.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::actor_identity::{ActorIdentity, ActorKind};
    ///
    /// let a = ActorIdentity::mint(ActorKind::Character, Utc::now());
    /// let b = ActorIdentity::mint(ActorKind::Character, Utc::now());
    /// assert_ne!(a.id, b.id);
    /// assert_eq!(a.did, None);
    /// ```
    pub fn mint(kind: ActorKind, now: DateTimeUtc) -> Self {
        Self {
            id: ActorIdentityId(uuid::Uuid::now_v7()),
            kind,
            did: None,
            state: ActorState::Active,
            handle: None,
            first_seen: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    /// Slice-1 base: every mint is a distinct row-to-be.
    #[test]
    fn mint_yields_distinct_ids() {
        assert_ne!(
            ActorIdentity::mint(ActorKind::User, Utc::now()).id,
            ActorIdentity::mint(ActorKind::User, Utc::now()).id
        );
    }

    /// The id round-trips through its stored UUID (the read-back path).
    #[test]
    fn id_rebuilds_from_stored_uuid() {
        let minted = ActorIdentity::mint(ActorKind::Account, Utc::now());
        assert_eq!(ActorIdentityId::new(*minted.id), minted.id);
    }

    /// Slice 2: every kind's stored spelling parses back to itself, and an
    /// unknown spelling is a loud error (a corrupted row, never a silent kind).
    #[test]
    fn kind_spelling_round_trips() {
        for kind in [ActorKind::User, ActorKind::Account, ActorKind::Character] {
            assert_eq!(ActorKind::try_from(kind.as_str()), Ok(kind));
        }
        assert_eq!(
            ActorKind::try_from("golem"),
            Err(UnknownActorKind("golem".to_string()))
        );
    }

    /// Slice 4: every state's stored spelling parses back; rows are born
    /// Active; an unknown spelling is a loud error.
    #[test]
    fn state_spelling_round_trips_and_mint_is_active() {
        for state in [
            ActorState::Active,
            ActorState::Pulled,
            ActorState::Tombstoned,
        ] {
            assert_eq!(ActorState::try_from(state.as_str()), Ok(state));
        }
        assert_eq!(
            ActorState::try_from("deleted"),
            Err(UnknownActorState("deleted".to_string()))
        );
        assert_eq!(
            ActorIdentity::mint(ActorKind::User, Utc::now()).state,
            ActorState::Active
        );
    }
}
