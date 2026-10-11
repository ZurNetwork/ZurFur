//! The identity resolver: a handle to its DID through DNS TXT, then the HTTPS
//! well-known file; a DID to its document, fetched and checked here; and the
//! both-ways rule that binds the two. Every fetch goes through the guarded client.

mod client;
mod document;
mod fetch;
mod handle;
mod limits;

pub use client::AtprotoIdentityResolver;
