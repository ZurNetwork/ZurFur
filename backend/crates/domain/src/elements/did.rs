//! Decentralized identifiers — the AT Protocol identity primitive, and the
//! identifier of every actor. A visitor's DID precedes the platform and is only
//! recognized; an account's is minted on its behalf by a
//! [`DidMinter`](crate::ports::DidMinter). The DID is the actor's only
//! identifier — there is no separate internal id behind it.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A decentralized identifier, held as the string the network gave us. Two ways
/// in, by provenance: [`From<String>`](From) wraps a DID from a trusted source
/// unchecked, and [`FromStr`] is the untrusted door, checking DID Core syntax
/// only. Treat the inner string as opaque.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::AsRef,
)]
#[as_ref(str)]
pub struct Did(String);

/// The DID's public-boundary lifecycle operations — the only sanctioned callers
/// of [`DidOperations`](crate::ports::DidOperations). Each is a separate
/// retryable step, run after the owning private transaction commits.
impl Did {
    /// Tombstone this DID on the PLC directory.
    pub async fn tombstone(
        &self,
        operations: &dyn crate::ports::DidOperations,
    ) -> anyhow::Result<()> {
        operations.tombstone(self).await
    }

    /// Re-point this DID's `alsoKnownAs` at `handle`.
    pub async fn update_handle(
        &self,
        handle: &crate::elements::handle::Handle,
        operations: &dyn crate::ports::DidOperations,
    ) -> anyhow::Result<()> {
        operations.update_handle(self, handle).await
    }
}

/// Why a string is not a DID. Carries only the offending input, so it is safe
/// to print.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DidParseError {
    #[error("not a DID (expected `did:<method>:<id>`): {0:?}")]
    InvalidInput(String),
}

impl FromStr for Did {
    type Err = DidParseError;

    /// DID Core syntax only: `did:` + method (`[a-z0-9]+`) + method-specific id
    /// (`[A-Za-z0-9._:%-]+`), all non-empty. Per-method shape is not checked.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let reject = || DidParseError::InvalidInput(text.to_string());
        let mut parts = text.splitn(3, ':');
        let (Some("did"), Some(method), Some(id)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(reject());
        };
        let method_ok = !method.is_empty()
            && method
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        let id_ok = !id.is_empty()
            && id.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':' | b'%')
            });
        if !(method_ok && id_ok) {
            return Err(reject());
        }
        Ok(Self(text.to_string()))
    }
}

#[cfg(test)]
mod tests;
