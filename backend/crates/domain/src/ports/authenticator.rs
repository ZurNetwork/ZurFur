use async_trait::async_trait;

use crate::elements::did::Did;
use crate::elements::handle::AtHandle;

/// Authenticates a visitor against their PDS, yielding the DID they already own.
/// The two methods mirror the OAuth handshake and speak only a handle, an opaque
/// redirect URL and a [`Did`], so the protocol library stays inside its adapter.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Begin sign-in for `handle`, binding the DID it resolves to; returns the
    /// PDS authorization URL to redirect the visitor to.
    async fn start(&self, handle: &AtHandle) -> anyhow::Result<String>;

    /// Complete the callback the PDS redirected back with, returning the
    /// authenticated visitor's DID. Errors with [`AccountMismatch`](super::AccountMismatch)
    /// when the visitor signed in as an account other than the one bound at start.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
    ) -> anyhow::Result<Did>;
}
