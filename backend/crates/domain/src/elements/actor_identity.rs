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

use std::ops::Deref;

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

/// One actor's row in the super-table. In this slice the identity **is** its id —
/// pure existence; every attribute (kind, optional DID, cached handle, liveness
/// state) lands in a later slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorIdentity {
    pub id: ActorIdentityId,
}

impl ActorIdentity {
    /// Mint a brand-new actor identity with a fresh UUIDv7 key.
    ///
    /// Pure: this only builds the value — persisting it is
    /// [`crate::ports::ActorIdentityWrites::create`]'s job. Each call mints a
    /// distinct identity.
    ///
    /// ```
    /// use domain::elements::actor_identity::ActorIdentity;
    ///
    /// let a = ActorIdentity::mint();
    /// let b = ActorIdentity::mint();
    /// assert_ne!(a.id, b.id);
    /// ```
    pub fn mint() -> Self {
        Self {
            id: ActorIdentityId(uuid::Uuid::now_v7()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slice-1 base: every mint is a distinct row-to-be.
    #[test]
    fn mint_yields_distinct_ids() {
        assert_ne!(ActorIdentity::mint().id, ActorIdentity::mint().id);
    }

    /// The id round-trips through its stored UUID (the read-back path).
    #[test]
    fn id_rebuilds_from_stored_uuid() {
        let minted = ActorIdentity::mint();
        assert_eq!(ActorIdentityId::new(*minted.id), minted.id);
    }
}
