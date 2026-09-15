/// A collection NSID — the reverse-DNS name of the lexicon a record belongs to.
/// A newtype, not a validating parser; a value read off the wire is validated by
/// the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Nsid(String);

impl Nsid {
    /// Wrap an NSID string.
    pub fn new(nsid: impl Into<String>) -> Self {
        Self(nsid.into())
    }

    /// The NSID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Nsid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A record key (`rkey`) — the per-collection identifier of a single record,
/// treated as an opaque string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordKey(String);

impl RecordKey {
    /// Wrap an rkey string.
    pub fn new(rkey: impl Into<String>) -> Self {
        Self(rkey.into())
    }

    /// The rkey as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
