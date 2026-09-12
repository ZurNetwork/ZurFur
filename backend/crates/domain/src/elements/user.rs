//! The [`User`] — Zurfur's record of a recognized visitor. A visitor's identity
//! precedes the platform, so Zurfur recognizes rather than registers: one DID
//! maps to one User forever, provisioned idempotently through
//! [`crate::ports::UserWrites`]. (DESIGN 786439)

use std::{ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    datetime::DateTimeUtc,
    elements::{did::Did, id::IdError},
};

/// The identity of a [`User`]: their [`Did`]. The DID IS the key — there is no
/// separate private surrogate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Did);

impl UserId {
    /// Wraps a [`Did`] as a user id. Deterministic: the same DID always yields
    /// the same id.
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
/// Zurfur first saw it. Holds no profile data — handle, display name and avatar
/// are user-owned and fetched from the PDS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    /// When Zurfur first recognized this DID — an explicit domain fact, since
    /// the id carries no timestamp.
    pub created_at: DateTimeUtc,
}

impl User {
    /// The act of first recognition: key the User on `did` and stamp the
    /// moment. A pure value — persisting it, and the one-DID-one-User rule, are
    /// [`crate::ports::UserWrites::provision`]'s job.
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
