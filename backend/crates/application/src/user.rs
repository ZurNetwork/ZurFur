//! Use cases about the acting User.

use domain::elements::{did::Did, profile::Profile};
use domain::ports::{ProfileCache, ProfileSource};

pub mod me;

/// Read-through resolution of a visitor's profile: a cache hit is served
/// without waking the PDS, a miss reads the PDS and caches the result, and a
/// PDS failure degrades to `None` rather than erroring.
///
/// The cache fill is pool-backed and best-effort — a documented exception to
/// the compile-enforced Unit of Work, and a `put` failure is
/// swallowed so a cache hiccup never fails the read.
pub(crate) async fn resolve_profile(
    cache: &dyn ProfileCache,
    source: &dyn ProfileSource,
    did: &Did,
) -> Option<Profile> {
    if let Ok(Some(profile)) = cache.get(did).await {
        return Some(profile);
    }
    match source.fetch(did).await {
        Ok(profile) => {
            let _ = cache.put(&profile).await;
            Some(profile)
        }
        Err(_) => None,
    }
}

/// User use cases, with the ports already bound.
#[derive(Clone, Copy)]
pub struct Users<'a> {
    ports: &'a crate::Ports,
}

impl<'a> Users<'a> {
    /// Bind the namespace to the bag.
    pub fn new(ports: &'a crate::Ports) -> Self {
        Self { ports }
    }

    /// The bag this namespace was built over.
    pub fn ports(&self) -> &'a crate::Ports {
        self.ports
    }
}

impl<'a> From<&'a crate::Ports> for Users<'a> {
    fn from(ports: &'a crate::Ports) -> Self {
        Self::new(ports)
    }
}

impl<'a> From<&'a crate::App> for Users<'a> {
    fn from(app: &'a crate::App) -> Self {
        Self::new(app.ports())
    }
}
