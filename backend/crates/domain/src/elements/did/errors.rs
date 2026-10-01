/// Why a string is not a DID. Carries only the offending input, so it is safe
/// to print.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DidParseError {
    #[error("not a DID (expected `did:<method>:<id>`): {0:?}")]
    InvalidInput(String),
}

#[cfg(test)]
mod tests;
