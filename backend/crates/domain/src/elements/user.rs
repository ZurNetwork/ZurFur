use std::ops::Deref;

use crate::{datetime::DateTimeUtc, elements::did::Did};

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
    pub fn recognize(did: Did, now: DateTimeUtc) -> Self {
        Self {
            id: UserId(uuid::Uuid::now_v7()),
            did,
            created_at: now,
        }
    }
}
