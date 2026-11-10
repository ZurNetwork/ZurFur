use super::{KeyId, Purpose};

/// Why a custodian refused to seal, open, tag or verify. Its `Display` names the kind
/// and at most the purpose: never a key id, a subject, a secret or unchecked text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CustodyError {
    /// The custodian holds no key for this purpose.
    #[error("no custody key is held for the purpose {purpose}")]
    PurposeNotHeld { purpose: Purpose },
    /// The envelope's version is not one the custodian reads.
    #[error("the envelope's version is unknown")]
    UnknownVersion,
    /// The envelope or tag is not in a shape the custodian reads.
    #[error("the envelope is malformed")]
    Malformed,
    /// A well-formed key id names no key the custodian holds.
    #[error("the key id names no key the custodian holds")]
    UnknownKeyId { key_id: KeyId },
    /// The envelope or tag fails authentication.
    #[error("the envelope or tag is not authentic")]
    Unauthentic,
    /// The stored value has not been rewrapped into the current envelope.
    #[error("the stored value has not been rewrapped")]
    NotRewrapped,
}

#[cfg(test)]
mod tests;
