/// Error source of an account write whose handle collides with one already
/// stored — live **or** soft-deleted, since the handle index spans both
///. Adapters return it so routes can `downcast_ref` and answer
/// `409` rather than a generic `500`.
#[derive(Debug, thiserror::Error)]
#[error("account handle already taken")]
pub struct HandleTaken;

/// Marker error: the supplied DID is already interned as a different actor kind.
/// One DID = one actor; routes downcast this to a
/// `409 did_belongs_to_another_actor`.
#[derive(Debug, thiserror::Error)]
#[error("the DID already belongs to another actor (kind '{existing_kind}')")]
pub struct DidBelongsToAnotherActor {
    /// The kind the DID is already interned as.
    pub existing_kind: String,
}

/// Why a [`PublicRecords`] operation failed — the XRPC outcome, classified so a
/// caller can tell unreachable from rejected from not-found.
#[derive(Debug, thiserror::Error)]
pub enum PublicRecordsError {
    /// The PDS could not be reached at all. A transient, retryable transport
    /// fault — nothing was written.
    #[error("PDS unreachable: {0}")]
    Unreachable(#[source] anyhow::Error),
    /// The PDS answered but refused the operation, carrying the atproto error
    /// name and HTTP status.
    #[error(
        "PDS rejected ({status} {error}){}",
        message.as_ref().map(|m| format!(": {m}")).unwrap_or_default()
    )]
    Rejected {
        /// The HTTP status the PDS returned.
        status: u16,
        /// The atproto error name (the `error` field of the XRPC error body).
        error: String,
        /// The optional human-readable detail (`message` field), if any.
        message: Option<String>,
    },
    /// The record failed structural validation before or at the write.
    #[error("invalid record: {0}")]
    InvalidRecord(String),
    /// The operation named a record that does not exist.
    #[error("record not found")]
    NotFound,
    /// Any other, unclassified failure, carried opaquely so nothing is swallowed.
    #[error("unexpected public-records error: {0}")]
    Unexpected(#[source] anyhow::Error),
}

/// Error source of an [`Authenticator::complete`](super::Authenticator::complete)
/// whose visitor signed in as an account other than the one the typed handle
/// named. The route downcasts it to its own callback error code.
#[derive(Debug, thiserror::Error)]
#[error("signed in as a different account than the handle named")]
pub struct AccountMismatch;

/// Error from an [`Authenticator::complete`](super::Authenticator::complete)
/// refused for its browser: no binding token, no live sign-in under the
/// `state`, or another sign-in's token. The route downcasts it to its own
/// callback error code.
#[derive(Debug, thiserror::Error)]
#[error("sign-in not started in this browser, or expired")]
pub struct BrowserMismatch;

/// Why an [`IdentityResolver`](super::IdentityResolver) lookup failed. The
/// message is terse and never echoes the handle or DID asked about; a cause
/// rides `source()`. Only [`ResolveError::Unavailable`] is worth retrying.
#[derive(thiserror::Error)]
pub enum ResolveError {
    /// Nothing names a DID: no record, an unknown or tombstoned DID, a document
    /// that is malformed or names another DID, or an identifier not resolved here.
    #[error("identity not found")]
    NotFound,
    /// The handle names a DID whose document does not claim the handle back.
    #[error("handle not confirmed by its DID")]
    NotConfirmed,
    /// A host broke a fetch rule: a forbidden address, plain http, another
    /// port, a redirect, a bad certificate, or an oversized body.
    #[error("identity lookup refused")]
    Refused(#[source] anyhow::Error),
    /// DNS or a host did not answer in time, answered with a server error, or a
    /// deadline ran out.
    #[error("identity lookup unavailable")]
    Unavailable(#[source] anyhow::Error),
}

/// Hand-written so it prints the variant only: a cause can name the handle or
/// host, and reaches a log only when a caller reads `source()` on purpose.
impl std::fmt::Debug for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match self {
            Self::NotFound => "NotFound",
            Self::NotConfirmed => "NotConfirmed",
            Self::Refused(_) => "Refused",
            Self::Unavailable(_) => "Unavailable",
        };
        f.write_str(variant)
    }
}

#[cfg(test)]
mod tests;
