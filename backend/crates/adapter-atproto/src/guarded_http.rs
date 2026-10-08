//! The guarded web client: the one way this crate reaches a host that a
//! visitor, a handle or a DID document names. A URL policy (https, port 443,
//! a public domain name), a DNS filter over an address table, no redirects, no
//! proxy, a body cap and a total timeout.

mod address;
mod client;
mod dns;
mod errors;
mod limits;
mod policy;
#[cfg(test)]
mod scripted;

pub(crate) use client::GuardedHttp;
#[cfg(test)]
pub(crate) use limits::Timeouts;
