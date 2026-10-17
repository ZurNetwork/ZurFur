use async_trait::async_trait;

use super::errors::ResolveError;
use crate::elements::did::Did;
use crate::elements::handle::AtHandle;

/// Resolves the identities of actors Zurfur does not own: a handle to its DID
/// and a DID to its handle, each answer confirmed both ways.
#[async_trait]
pub trait IdentityResolver: Send + Sync {
    /// The DID `handle` names, returned only when that DID's document claims the handle back.
    async fn resolve_handle(&self, handle: &AtHandle) -> Result<Did, ResolveError>;

    /// The handle `did` claims, when it checks out both ways; `None` when it does not.
    /// Errors when the DID itself does not resolve, or the check could not finish.
    async fn resolve_did(&self, did: &Did) -> Result<Option<AtHandle>, ResolveError>;
}
