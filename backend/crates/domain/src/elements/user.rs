//! The [`User`] — Zurfur's record of a recognized visitor.
//!
//! A visitor's identity lives on their PDS and precedes the platform, so Zurfur
//! *recognizes* rather than registers: the first time a [`Did`] signs in we mint
//! a [`User`] for it; thereafter that DID maps to that same User forever. See
//! [`crate::ports::UserRepo`] for the idempotent provisioning port, ZMVP-9, and
//! DESIGN/User.

use std::ops::Deref;

use crate::{datetime::DateTimeUtc, elements::did::Did};

/// The app-private, stable handle for a [`User`].
///
/// A UUIDv7 wrapped for type safety, so a user id can't be passed where some
/// other id is wanted. Public callers (sessions, foreign keys) hold this; the
/// public-facing identity is the user's [`Did`]. Deref exposes the inner UUID.
///
/// References: [`new`](UserId::new), [`User::recognize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(uuid::Uuid);

impl UserId {
    /// Rebuilds an id from its stored UUID — e.g. a row read back from Postgres.
    /// Minting a *fresh* id happens in [`User::recognize`], not here.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for UserId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A recognized visitor: the binding of a public [`Did`] to an app-private
/// [`UserId`], stamped with when Zurfur first saw it.
///
/// One DID maps to one User forever (see [`crate::ports::UserRepo::provision`]).
/// The struct holds no profile data — handle, display name, and avatar are
/// user-owned, fetched live from the PDS via [`crate::ports::ProfileSource`]
/// (DESIGN/User, ZMVP-9/10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub did: Did,
    /// When Zurfur first recognized this DID. Stored as an explicit domain fact,
    /// not derived from the UUIDv7 id: import flows can make recognition time
    /// diverge from key-minting time.
    pub created_at: DateTimeUtc,
}

impl User {
    /// The act of first recognition: mint a fresh UUIDv7 key and stamp the
    /// moment. `now` is injected so tests and import flows stay deterministic.
    ///
    /// Pure: this only builds the value — persisting it (and enforcing the
    /// one-DID-one-User rule) is [`crate::ports::UserRepo::provision`]'s job.
    /// Each call mints a *new* id, so calling it twice for the same DID yields
    /// two distinct Users; go through the repo to recognize idempotently.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{did::Did, user::User};
    ///
    /// let user = User::recognize(Did::new("did:plc:example".to_string()), Utc::now());
    /// assert_eq!(&**user.did, "did:plc:example");
    /// ```
    pub fn recognize(did: Did, now: DateTimeUtc) -> Self {
        Self {
            id: UserId(uuid::Uuid::now_v7()),
            did,
            created_at: now,
        }
    }
}
