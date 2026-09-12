//! The [`User`] — Zurfur's record of a recognized visitor.
//!
//! A visitor's identity lives on their PDS and precedes the platform, so Zurfur
//! *recognizes* rather than registers: the first time a [`Did`] signs in we mint
//! a [`User`] for it; thereafter that DID maps to that same User forever. See
//! [`crate::ports::UserWrites`] for the idempotent provisioning port, ZMVP-9, and
//! DESIGN/User.

use std::{ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    datetime::DateTimeUtc,
    elements::{did::Did, id::IdError},
};

/// The identity of a [`User`]: their [`Did`], wrapped so a user id cannot be
/// passed where some other actor's id is wanted.
///
/// The DID *is* the key — there is no separate private surrogate (DD
/// `57081857`). `Deref` exposes the inner [`Did`].
///
/// References: [`new`](UserId::new), [`User::recognize`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Did);

impl UserId {
    /// Wraps a [`Did`] as a user id — a session subject, a row read back from
    /// Postgres, a DID freshly minted for this actor. Deterministic: the same
    /// DID always yields the same id.
    pub fn new(id: Did) -> Self {
        Self(id)
    }
}

impl Deref for UserId {
    type Target = Did;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for UserId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let did = s
            .to_string()
            .parse::<Did>()
            .map(Self)
            .map_err(|_| IdError::ParsingError)?;

        Ok(did)
    }
}

impl From<Did> for UserId {
    fn from(value: Did) -> Self {
        Self(value)
    }
}

/// A recognized visitor: their [`Did`] as a [`UserId`], stamped with when
/// Zurfur first saw it.
///
/// One DID maps to one User forever (see [`crate::ports::UserWrites::provision`]).
/// The struct holds no profile data — handle, display name, and avatar are
/// user-owned, fetched live from the PDS via [`crate::ports::ProfileSource`]
/// (DESIGN/User, ZMVP-9/10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    /// When Zurfur first recognized this DID. Stored as an explicit domain
    /// fact: the id carries no timestamp, and import flows can make recognition
    /// time diverge from when the DID itself was minted.
    pub created_at: DateTimeUtc,
}

impl User {
    /// The act of first recognition: key the User on `did` and stamp the
    /// moment. `now` is injected so tests and import flows stay deterministic.
    ///
    /// Pure: this only builds the value — persisting it (and enforcing the
    /// one-DID-one-User rule) is [`crate::ports::UserWrites::provision`]'s job.
    /// The id is the DID, so calling it twice for one DID yields two Users that
    /// differ only in `created_at`; go through the repo to recognize idempotently.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{did::Did, user::User};
    ///
    /// let user = User::recognize(Did::new("did:plc:example".to_string()), Utc::now());
    /// assert_eq!(&**user.id, "did:plc:example");
    /// ```
    pub fn recognize(did: Did, now: DateTimeUtc) -> Self {
        Self {
            id: UserId::new(did),
            created_at: now,
        }
    }
}
