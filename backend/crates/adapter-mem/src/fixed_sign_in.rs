//! The fixed sign-in: an [`Authenticator`] that signs every browser in as one
//! DID, through the same browser-binding handshake the real one keeps.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
/// callback with a fresh browser binding; `complete` takes each binding once,
/// within its lifetime.
pub struct FixedSignIn {
    /// The DID every completed sign-in answers.
    did: Did,
    /// The absolute sign-in callback URL `start` redirects to.
    callback: String,
    /// How long a binding stays valid after `start` issues it.
    lifetime: Duration,
    /// Bindings `start` handed out that no `complete` has taken yet, each with
    /// when it was issued.
    issued: Mutex<HashMap<String, Instant>>,
}

impl FixedSignIn {
    /// A sign-in that answers `did` and redirects to `callback`, the absolute
    /// URL of the sign-in callback route.
    pub fn new(did: Did, callback: impl Into<String>) -> Self {
        Self {
            did,
            callback: callback.into(),
            lifetime: SIGN_IN_LIFETIME,
            issued: Mutex::new(HashMap::new()),
        }
    }

    /// Whether a binding issued at `issued_at` is still valid at `now`.
    fn is_live(&self, issued_at: Instant, now: Instant) -> bool {
        now.duration_since(issued_at) < self.lifetime
    }
}

#[async_trait]
impl Authenticator for FixedSignIn {
    async fn start(&self, _handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        let binding = uuid::Uuid::now_v7().to_string();
        let now = Instant::now();
        {
            let mut issued = self
                .issued
                .lock()
                .expect("FixedSignIn issued mutex poisoned");
            // Every start prunes the bindings that expired unused.
            issued.retain(|_, issued_at| self.is_live(*issued_at, now));
            issued.insert(binding.clone(), now);
        }
        let started = SigninStarted {
            authorization_url: format!("{}?code={CALLBACK_CODE}", self.callback),
            browser_binding: BrowserBinding::from(binding),
            lifetime: self.lifetime,
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
        let issued_at = self
            .issued
            .lock()
            .expect("FixedSignIn issued mutex poisoned")
            .remove(binding.as_ref())
            .ok_or_else(|| {
                anyhow::anyhow!("the browser's sign-in binding was not issued, or was already used")
            })?;
        if !self.is_live(issued_at, Instant::now()) {
            anyhow::bail!("the browser's sign-in binding has expired");
        }
        Ok(self.did.clone())
    }
}

#[cfg(test)]
mod tests;
