//! Decentralized identifiers — the AT Protocol identity primitive.
//!
//! A DID (`did:plc:…`, `did:web:…`) is the stable, self-sovereign id of an actor
//! on the network. On Zurfur a *visitor's* DID precedes the platform and is only
//! ever recognized, never minted (see DESIGN/User, DESIGN/"DID:PLC vs DID:Web");
//! an *account's* DID is minted on its behalf by a `DidMinter`
//! (see [`crate::ports::DidMinter`]).

use std::ops::Deref;
use std::str::FromStr;

/// A decentralized identifier, held as the string the network gave us.
///
/// Two ways in, by provenance (Engineer ruling 2026-08-24):
/// - [`Did::new`] wraps a DID from a **trusted** source unchecked — the PDS at
///   sign-in, our own store, a `DidMinter`. The domain never originates a DID,
///   so those sources are the norm and pay no parse.
/// - [`FromStr`] (`text.parse::<Did>()`) is the **untrusted** door — a file on
///   disk, a command-line argument, anything a person typed. It checks the
///   DID Core syntax (`did:<method>:<id>`), nothing per-method: the domain
///   must not reject a DID the network considers valid.
///
/// Treat the inner string as opaque; deref to read it.
///
/// References: [`new`](Did::new), [`crate::elements::user::User`],
/// [`crate::ports::DidMinter`], DESIGN/User; DID Core §3.1 (syntax).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Did(String);

impl Did {
    /// Wraps a DID the caller already trusts — sourced from the PDS at sign-in,
    /// read back from our own store, or minted by a `DidMinter`. No validation:
    /// for untrusted text use [`str::parse`] (the [`FromStr`] impl) instead.
    pub fn new(did: String) -> Self {
        Self(did)
    }
}

/// Why a string is not a DID. Carries the offending input so a caller can
/// name it; nothing else, so it is safe to print.
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

    /// DID Core syntax only: `did:` + method (`[a-z0-9]+`) + method-specific
    /// id (`[A-Za-z0-9._:%-]+`), all non-empty. Per-method shape (e.g. plc's
    /// base32 length) is deliberately NOT checked here.
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
