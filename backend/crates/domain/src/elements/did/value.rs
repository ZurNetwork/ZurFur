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

impl FromStr for Did {
    type Err = super::DidParseError;

    /// DID Core syntax only: `did:` + method (`[a-z0-9]+`) + method-specific id
    /// (`[A-Za-z0-9._:%-]+`), all non-empty. Per-method shape is not checked.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let reject = || super::DidParseError::InvalidInput(text.to_string());
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
