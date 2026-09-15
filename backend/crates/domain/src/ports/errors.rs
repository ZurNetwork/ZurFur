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

/// Error source of an element write whose tab does not exist in that commission
/// — an absent tab id and one belonging to another commission, indistinguishably,
/// so probing tab ids reveals nothing. Adapters return it so the route can
/// `downcast_ref` and answer `404`.
#[derive(Debug, thiserror::Error)]
#[error("tab not found in this commission")]
pub struct UnknownTab;

/// Error source of an element write whose surface the composition
/// [`SKELETON`](crate::elements::commission::SKELETON) does not declare **in that
/// tab** — the refusal is about the pair, not the surface alone. Surfaces are
/// code-declared and global, so this leaks nothing and routes answer `422`.
/// Adapters resolve the tab first, so an address wrong in both ways refuses as
/// [`UnknownTab`].
#[derive(Debug, thiserror::Error)]
#[error("no such declared surface")]
pub struct UnknownSurface;

/// Error source of [`CommissionWrites::remove_element`] when the element does not
/// exist in that commission — an absent id and one belonging to another
/// commission, indistinguishably. Adapters return it so the route can
/// `downcast_ref` and answer `404`. There is no "cannot remove" sibling: every
/// element is removable by construction.
#[derive(Debug, thiserror::Error)]
#[error("element not found in this commission")]
pub struct ElementNotFound;
