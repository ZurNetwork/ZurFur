//! The guarded web client: the one way this crate reaches a host that a
//! visitor, a handle or a DID document names. A URL policy (https, port 443,
//! a public domain name), a DNS filter over an address table, no redirects, no
//! proxy, a body cap and a total timeout. Its system DNS also answers the
//! identity resolver's TXT lookups.

mod address;
mod client;
mod dns;
mod errors;
mod limits;
mod policy;
#[cfg(test)]
mod scripted;

pub(crate) use client::{GuardedHttp, causes};
pub(crate) use dns::{LookupError, SystemDns, TxtLookup};
pub(crate) use errors::FetchError;
#[cfg(test)]
pub(crate) use errors::Refusal;
pub(crate) use limits::DNS_TIMEOUT;
#[cfg(test)]
pub(crate) use limits::Timeouts;
pub(crate) use policy::{PublicHttpsUrl, is_public_domain};
#[cfg(test)]
pub(crate) use scripted::{Script, ScriptedLookup, TxtScript};
