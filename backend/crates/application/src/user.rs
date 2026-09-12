//! Use cases about the acting User.

pub mod me;

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
