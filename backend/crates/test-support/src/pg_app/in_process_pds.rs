use std::sync::Mutex;

use async_trait::async_trait;
use domain::elements::{did::Did, profile::Profile};
use domain::ports::{Authenticator, ProfileSource};

/// An in-process stand-in for each visitor's PDS: it signs in the handles
/// registered with it, as their DIDs, and answers their profiles. A handle or
/// DID it was never given is refused, as an unreachable PDS would be.
#[derive(Debug, Default)]
pub struct InProcessPds {
    /// One profile per registered handle and per DID; a later registration
    /// replaces any earlier one sharing its handle or its DID.
    profiles: Mutex<Vec<Profile>>,
}

impl InProcessPds {
    /// Make `profile` known: its handle signs in as its DID, and its DID's
    /// profile fetch answers it.
    pub fn register(&self, profile: Profile) {
        let mut profiles = self.profiles.lock().expect("InProcessPds mutex poisoned");
        profiles.retain(|known| known.handle != profile.handle && known.did != profile.did);
        profiles.push(profile);
    }

    /// The DID registered under `handle`, if any.
    fn did_for(&self, handle: &str) -> Option<Did> {
        self.profiles
            .lock()
            .expect("InProcessPds mutex poisoned")
            .iter()
            .find(|known| known.handle.as_ref() == handle)
            .map(|known| known.did.clone())
    }
}

#[async_trait]
impl Authenticator for InProcessPds {
    /// The authorization URL is the app's own callback, its code the handle.
    async fn start(&self, handle: &str) -> anyhow::Result<String> {
        self.did_for(handle)
            .ok_or_else(|| anyhow::anyhow!("no in-process PDS knows the handle {handle}"))?;
        let callback = format!("/signin-callback?code={handle}");
        Ok(callback)
    }

    async fn complete(
        &self,
        code: String,
        _state: Option<String>,
        _iss: Option<String>,
    ) -> anyhow::Result<Did> {
        self.did_for(&code)
            .ok_or_else(|| anyhow::anyhow!("the in-process PDS issued no code {code}"))
    }
}

#[async_trait]
impl ProfileSource for InProcessPds {
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        self.profiles
            .lock()
            .expect("InProcessPds mutex poisoned")
            .iter()
            .find(|known| &known.did == did)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no in-process PDS hosts {did}"))
    }
}

#[cfg(test)]
mod tests;
