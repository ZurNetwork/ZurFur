use std::time::Duration;

use async_trait::async_trait;

use crate::elements::did::Did;
use crate::elements::handle::AtHandle;

/// Authenticates a visitor against their PDS, yielding the DID they already own.
/// The two methods mirror the OAuth handshake and speak only a handle, an opaque
/// redirect URL, a browser token and a [`Did`], so the protocol library stays
/// inside its adapter.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Begin sign-in for `handle`, binding the DID it resolves to and the
    /// browser that asked: the browser must hold the returned token at the callback.
    async fn start(&self, handle: &AtHandle) -> anyhow::Result<SigninStarted>;

    /// Complete the callback the PDS redirected back with, returning the
    /// authenticated visitor's DID. Refused unless `browser_binding` is the
    /// token `start` returned for this `state`; errors with
    /// [`AccountMismatch`](super::AccountMismatch) when the visitor signed in as
    /// another account than the one bound at start.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did>;
}

/// A started sign-in: where to send the visitor, and the token their browser
/// must keep until the callback, valid for `lifetime`.
#[derive(Debug)]
pub struct SigninStarted {
    /// The PDS authorization URL to redirect the visitor to.
    pub authorization_url: String,
    /// The token binding this sign-in to the browser that started it.
    pub browser_binding: BrowserBinding,
    /// How long the sign-in, and so the token, stays valid.
    pub lifetime: Duration,
}

/// A random token tying a sign-in to the browser that started it. Opaque, and
/// its `Debug` never shows the value, so it cannot reach a log. `From<String>`
/// wraps a token as the browser presented it.
#[derive(Clone, derive_more::From, derive_more::AsRef)]
#[as_ref(str)]
pub struct BrowserBinding(String);

/// Hand-written so the token never prints: only the type's name does.
impl std::fmt::Debug for BrowserBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserBinding(..)")
    }
}

#[cfg(test)]
mod tests;
