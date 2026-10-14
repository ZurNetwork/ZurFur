//! Use cases about the acting User.

use crate::ports::WithPorts;

pub(crate) mod den;
pub mod me;
mod profile_lookup;

pub use profile_lookup::{FAILED_LOOKUP_HOLD, LOOKUP_CAP, PROFILE_WAIT, ProfileLookups};

/// User use cases, with the ports already bound.
#[derive(Clone, Copy, WithPorts)]
pub struct Users<'a> {
    #[ports(is_root = true)]
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
