use crate::elements::did::Did;

use super::{AtUriParseError, Nsid, RecordKey};

/// The fully-qualified address of a record in an actor's repo:
/// `at://<did>/<collection>/<rkey>` — the
/// [AT-URI](https://atproto.com/specs/at-uri-scheme) restricted to the
/// repo-record form, with no query or fragment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AtUri {
    /// The repo owner's DID (the URI authority).
    pub did: Did,
    /// The collection NSID.
    pub collection: Nsid,
    /// The record key within the collection.
    pub rkey: RecordKey,
}

impl AtUri {
    /// Build an AT-URI from its parts.
    pub fn new(did: Did, collection: Nsid, rkey: RecordKey) -> Self {
        Self {
            did,
            collection,
            rkey,
        }
    }

    /// Parse an `at://<did>/<collection>/<rkey>` string. None of the three
    /// parts may contain `/`, `?` or `#`, so they split unambiguously; a query
    /// or fragment is rejected outright.
    pub fn parse(s: &str) -> Result<Self, AtUriParseError> {
        let rest = s
            .strip_prefix("at://")
            .ok_or(AtUriParseError::MissingScheme)?;
        if rest.contains(['?', '#']) {
            return Err(AtUriParseError::Malformed);
        }
        let mut parts = rest.splitn(3, '/');
        let (Some(did), Some(collection), Some(rkey)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(AtUriParseError::Malformed);
        };
        if did.is_empty() || collection.is_empty() || rkey.is_empty() || rkey.contains('/') {
            return Err(AtUriParseError::Malformed);
        }
        Ok(Self {
            did: Did::from(did.to_string()),
            collection: Nsid::new(collection),
            rkey: RecordKey::new(rkey),
        })
    }
}

impl std::fmt::Display for AtUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at://{}/{}/{}", self.did, self.collection, self.rkey)
    }
}

impl std::str::FromStr for AtUri {
    type Err = AtUriParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests;
