use crate::datetime::DateTimeUtc;
use crate::elements::did::Did;

use super::{ActorIdentityId, ActorKind, ActorState};

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
