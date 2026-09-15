use async_trait::async_trait;

use crate::elements::did::Did;

/// Authenticates a visitor against their PDS, yielding the DID they already own.
/// The two methods mirror the OAuth handshake and speak only a handle, an opaque
/// redirect URL and a [`Did`], so the protocol library stays inside its adapter.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Begin sign-in for a handle; returns the PDS authorization URL to redirect
    /// the visitor to.
    async fn start(&self, handle: &str) -> anyhow::Result<String>;

    /// Complete the callback the PDS redirected back with, returning the
    /// authenticated visitor's DID.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
    ) -> anyhow::Result<Did>;
}
