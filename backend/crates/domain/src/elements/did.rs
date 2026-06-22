//! Decentralized identifiers — the AT Protocol identity primitive.
//!
//! A DID (`did:plc:…`, `did:web:…`) is the stable, self-sovereign id of an actor
//! on the network. On Zurfur a *visitor's* DID precedes the platform and is only
//! ever recognized, never minted (see DESIGN/User, DESIGN/"DID:PLC vs DID:Web");
//! an *account's* DID is minted on its behalf by a `DidMinter`
//! (see [`crate::ports::DidMinter`]).

use std::ops::Deref;

/// A decentralized identifier, held as the opaque string the network gave us.
///
/// The wrapper is a newtype for type safety, not a parser: there is deliberately
/// no validating constructor, because the domain never originates a DID — it only
/// ever holds one sourced from a trusted place (a PDS at sign-in, our own store,
/// or a `DidMinter`). Treat the inner string as opaque; deref to read it.
///
/// References: [`new`](Did::new), [`crate::elements::user::User`],
/// [`crate::ports::DidMinter`], DESIGN/User.
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
