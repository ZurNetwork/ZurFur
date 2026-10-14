//! The fixed sign-in: an [`Authenticator`] that signs every browser in as one
//! DID, through the same browser-binding handshake the real one keeps.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use domain::elements::did::Did;
use domain::elements::handle::AtHandle;
use domain::ports::{Authenticator, BrowserBinding, SigninStarted};

/// The code the fixed sign-in's callback carries; `complete` never reads it.
const CALLBACK_CODE: &str = "fixed-sign-in";

/// How long a started sign-in, and so its browser binding, stays valid.
const SIGN_IN_LIFETIME: Duration = Duration::from_secs(600);

/// An [`Authenticator`] whose every completed sign-in is one fixed DID,
/// whatever handle was typed. `start` sends the browser straight back to the
/// callback with a fresh browser binding; `complete` takes each binding once.
pub struct FixedSignIn {
    /// The DID every completed sign-in answers.
    did: Did,
    /// The absolute sign-in callback URL `start` redirects to.
    callback: String,
    /// Bindings `start` handed out that no `complete` has taken yet.
    issued: Mutex<HashSet<String>>,
}

impl FixedSignIn {
    /// A sign-in that answers `did` and redirects to `callback`, the absolute
    /// URL of the sign-in callback route.
    pub fn new(did: Did, callback: impl Into<String>) -> Self {
        Self {
            did,
            callback: callback.into(),
            issued: Mutex::new(HashSet::new()),
        }
    }
}

#[async_trait]
impl Authenticator for FixedSignIn {
    async fn start(&self, _handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        let binding = uuid::Uuid::now_v7().to_string();
        self.issued
            .lock()
            .expect("FixedSignIn issued mutex poisoned")
            .insert(binding.clone());
        let started = SigninStarted {
            authorization_url: format!("{}?code={CALLBACK_CODE}", self.callback),
            browser_binding: BrowserBinding::from(binding),
            lifetime: SIGN_IN_LIFETIME,
        };
        Ok(started)
    }

    async fn complete(
        &self,
        _code: String,
        _state: Option<String>,
        _iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did> {
        let binding = browser_binding
            .ok_or_else(|| anyhow::anyhow!("the browser presented no sign-in binding"))?;
        let was_issued = self
            .issued
            .lock()
            .expect("FixedSignIn issued mutex poisoned")
            .remove(binding.as_ref());
        if !was_issued {
            anyhow::bail!("the browser's sign-in binding was not issued, or was already used");
        }
        Ok(self.did.clone())
    }
}

#[cfg(test)]
mod tests;
