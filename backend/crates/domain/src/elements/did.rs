use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Did(String);

impl Did {
    /// Wraps a DID the caller already trusts — sourced from the PDS at sign-in,
    /// or read back from our own store. The platform never mints DIDs, so there
    /// is deliberately no validating constructor here.
    pub fn new(did: String) -> Self {
        Self(did)
    }
}

impl Deref for Did {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
