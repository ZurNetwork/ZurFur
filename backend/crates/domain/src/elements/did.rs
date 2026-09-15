//! Decentralized identifiers — the AT Protocol identity primitive, and the
//! identifier of every actor. A visitor's DID precedes the platform and is only
//! recognized; an account's is minted on its behalf by a
//! [`DidMinter`](crate::ports::DidMinter). The DID is the actor's only
//! identifier — there is no separate internal id behind it.

mod errors;
mod value;

pub use errors::DidParseError;
pub use value::Did;
