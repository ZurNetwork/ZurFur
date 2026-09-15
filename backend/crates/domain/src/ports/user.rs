use async_trait::async_trait;

use crate::elements::did::Did;
use crate::elements::user::{User, UserId};

/// The write surface of Zurfur's record of recognized visitors — reachable only
/// on an open [`UnitOfWork`].
#[async_trait]
pub trait UserWrites: Send {
    /// Recognize a DID: the first call mints a User, every later call returns
    /// that same User. One DID, one User, forever — idempotent.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User>;
}

/// The read surface of Zurfur's record of recognized visitors — pool-backed and
/// non-transactional; recognition (the write) lives on [`UserWrites`].
#[async_trait]
pub trait UserStore: Send + Sync {
    /// Resolve a stored UserId back to its User, or `None` if no such User
    /// exists. Never touches the PDS.
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>>;

    /// Resolve a DID to its User *without minting one*, or `None` if no User has
    /// ever been recognized for it.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>>;
}
