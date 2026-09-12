//! The [`ActorIdentity`] — a row in the actor super-table: one row per actor the
//! Index has ever seen, and the single table every actor reference FKs into.
//!
//! Rows are immortal: the port exposes no delete, so liveness is an
//! [`ActorState`] on the row and an FK into `actor_identity` can never break.
//! [`ActorKind`] is the closed vocabulary that, with `UNIQUE (id, kind)`, every
//! kind-checked reference's composite FK targets. A row's `handle` is a
//! refreshable display cache, never a claim-validated handle, and `first_seen`
//! is immutable.

use std::ops::Deref;
use std::str::FromStr;

use crate::datetime::DateTimeUtc;
use crate::elements::did::Did;
use crate::elements::id::{IdError, parse_uuid};

/// The app-private key of an [`ActorIdentity`] row (UUIDv7) — the anchor every
/// kind-checked actor reference FKs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorIdentityId(uuid::Uuid);

impl ActorIdentityId {
    /// Rebuilds an id from its stored UUID; a fresh one is minted by
    /// [`ActorIdentity::mint`].
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

impl FromStr for ActorIdentityId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

/// What kind of actor an identity row is — the closed vocabulary. A seated
/// Golem acts as a User, so there is no `golem` variant.
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

    /// Parse the stored spelling back; an error means a corrupted row.
    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "user" => Ok(ActorKind::User),
            "account" => Ok(ActorKind::Account),
            "character" => Ok(ActorKind::Character),
            other => Err(UnknownActorKind(other.to_string())),
        }
    }
}

/// An actor identity's liveness — a state on the immortal row, never a removal.
/// Identity is permanent and FK-enforced; liveness is consulted per-read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorState {
    /// The normal case: the actor is live.
    Active,
    /// The DID/PDS stopped resolving — the reference is kept, content absent.
    Pulled,
    /// Deleted: the identity is anonymized, the facts referencing it stay.
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

    /// Parse the stored spelling back; an error means a corrupted row.
    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "active" => Ok(ActorState::Active),
            "pulled" => Ok(ActorState::Pulled),
            "tombstoned" => Ok(ActorState::Tombstoned),
            other => Err(UnknownActorState(other.to_string())),
        }
    }
}

/// One actor's row in the super-table: its id, kind, optional [`Did`],
/// liveness, cached handle, and when the Index first saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorIdentity {
    pub id: ActorIdentityId,
    pub kind: ActorKind,
    /// The actor's DID, when it has one. Unique where present — one DID is one
    /// actor, ever, DB-enforced.
    pub did: Option<Did>,
    /// Liveness; every row is born [`ActorState::Active`].
    pub state: ActorState,
    /// A refreshable display cache of the actor's atproto handle — foreign
    /// data, so a plain string, never the claim-validated
    /// [`Handle`](crate::elements::handle::Handle). Rows are born uncached.
    pub handle: Option<String>,
    /// When the Index first saw this actor — injected, and immutable.
    pub first_seen: DateTimeUtc,
}

impl ActorIdentity {
    /// Mint a DID-less actor identity of `kind` with a fresh UUIDv7 key, first
    /// seen `now`. A pure value — persisting it is
    /// [`create`](crate::ports::ActorIdentityWrites::create)'s job, and the
    /// store's per-kind CHECK refuses a DID-less user or account. DID-bearing
    /// actors go through [`intern`](crate::ports::ActorIdentityWrites::intern).
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

    /// Every mint is a distinct row-to-be.
    #[test]
    fn mint_yields_distinct_ids() {
        assert_ne!(
            ActorIdentity::mint(ActorKind::User, Utc::now()).id,
            ActorIdentity::mint(ActorKind::User, Utc::now()).id
        );
    }

    /// The id round-trips through its stored UUID.
    #[test]
    fn id_rebuilds_from_stored_uuid() {
        let minted = ActorIdentity::mint(ActorKind::Account, Utc::now());
        assert_eq!(ActorIdentityId::new(*minted.id), minted.id);
    }

    /// Every kind's spelling parses back; an unknown one is a loud error.
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

    /// Every state's spelling parses back, and rows are born Active.
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
