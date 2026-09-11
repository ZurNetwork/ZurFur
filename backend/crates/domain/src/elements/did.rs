//! Decentralized identifiers — the AT Protocol identity primitive, and the
//! identifier of every actor. A visitor's DID precedes the platform and is only
//! recognized; an account's is minted on its behalf by a
//! [`DidMinter`](crate::ports::DidMinter). (DD 4358151, DD 57081857)

use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A decentralized identifier, held as the string the network gave us. Two ways
/// in, by provenance: [`Did::new`] wraps a DID from a trusted source unchecked,
/// and [`FromStr`] is the untrusted door, checking DID Core syntax only. Treat
/// the inner string as opaque.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Did(String);

impl Did {
    /// Wraps a DID the caller already trusts. No validation — for untrusted
    /// text use [`str::parse`] instead.
    pub fn new(did: String) -> Self {
        Self(did)
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DidParseError {
    input: String,
}

impl std::fmt::Display for DidParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "not a DID (expected `did:<method>:<id>`): {:?}",
            self.input
        )
    }
}

impl std::error::Error for DidParseError {}

impl FromStr for Did {
    type Err = DidParseError;

    /// DID Core syntax only: `did:` + method (`[a-z0-9]+`) + method-specific id
    /// (`[A-Za-z0-9._:%-]+`), all non-empty. Per-method shape is not checked.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let reject = || DidParseError {
            input: text.to_string(),
        };
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

impl Deref for Did {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Did {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_formed_dids_parse() {
        for text in [
            "did:plc:ewvi7nxzyoun6zhxrhs64oiz",
            "did:web:example.com",
            "did:web:example.com:user:alice",
            "did:key:z6Mk",
        ] {
            let did: Did = text.parse().unwrap();
            assert_eq!(&*did, text);
        }
    }

    #[test]
    fn malformed_dids_are_refused() {
        for text in [
            "",
            "did:",
            "did:plc:",
            "plc:abc",
            "did:PLC:abc",
            "did:plc:a b",
            "DID:plc:abc",
            "did::abc",
            "'; DROP TABLE users; --",
        ] {
            assert!(text.parse::<Did>().is_err(), "{text:?} must not parse");
        }
    }

    #[test]
    fn the_error_names_the_input_and_nothing_else() {
        let error = "nope".parse::<Did>().unwrap_err();
        assert_eq!(
            error.to_string(),
            "not a DID (expected `did:<method>:<id>`): \"nope\""
        );
    }
}
