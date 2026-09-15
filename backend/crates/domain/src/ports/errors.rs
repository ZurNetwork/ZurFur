/// Error source of an account write whose handle collides with one already
/// stored — live **or** soft-deleted, since the handle index spans both
///. Adapters return it so routes can `downcast_ref` and answer
/// `409` rather than a generic `500`.
#[derive(Debug)]
pub struct HandleTaken;

impl std::fmt::Display for HandleTaken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "account handle already taken")
    }
}

impl std::error::Error for HandleTaken {}

/// Marker error: the supplied DID is already interned as a different actor kind.
/// One DID = one actor; routes downcast this to a
/// `409 did_belongs_to_another_actor`.
#[derive(Debug)]
pub struct DidBelongsToAnotherActor {
    /// The kind the DID is already interned as.
    pub existing_kind: String,
}

impl std::fmt::Display for DidBelongsToAnotherActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the DID already belongs to another actor (kind '{}')",
            self.existing_kind
        )
    }
}

impl std::error::Error for DidBelongsToAnotherActor {}

/// Why a [`PublicRecords`] operation failed — the XRPC outcome, classified so a
/// caller can tell unreachable from rejected from not-found.
#[derive(Debug)]
pub enum PublicRecordsError {
    /// The PDS could not be reached at all. A transient, retryable transport
    /// fault — nothing was written.
    Unreachable(anyhow::Error),
    /// The PDS answered but refused the operation, carrying the atproto error
    /// name and HTTP status.
    Rejected {
        /// The HTTP status the PDS returned.
        status: u16,
        /// The atproto error name (the `error` field of the XRPC error body).
        error: String,
        /// The optional human-readable detail (`message` field), if any.
        message: Option<String>,
    },
    /// The record failed structural validation before or at the write.
    InvalidRecord(String),
    /// The operation named a record that does not exist.
    NotFound,
    /// Any other, unclassified failure, carried opaquely so nothing is swallowed.
    Unexpected(anyhow::Error),
}

impl std::fmt::Display for PublicRecordsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublicRecordsError::Unreachable(e) => write!(f, "PDS unreachable: {e}"),
            PublicRecordsError::Rejected {
                status,
                error,
                message,
            } => match message {
                Some(m) => write!(f, "PDS rejected ({status} {error}): {m}"),
                None => write!(f, "PDS rejected ({status} {error})"),
            },
            PublicRecordsError::InvalidRecord(why) => write!(f, "invalid record: {why}"),
            PublicRecordsError::NotFound => write!(f, "record not found"),
            PublicRecordsError::Unexpected(e) => write!(f, "unexpected public-records error: {e}"),
        }
    }
}

impl std::error::Error for PublicRecordsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PublicRecordsError::Unreachable(e) | PublicRecordsError::Unexpected(e) => {
                Some(e.as_ref())
            }
            _ => None,
        }
    }
}

/// Error source of an element write whose tab does not exist in that commission
/// — an absent tab id and one belonging to another commission, indistinguishably,
/// so probing tab ids reveals nothing. Adapters return it so the route can
/// `downcast_ref` and answer `404`.
#[derive(Debug)]
pub struct UnknownTab;

impl std::fmt::Display for UnknownTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tab not found in this commission")
    }
}

impl std::error::Error for UnknownTab {}

/// Error source of an element write whose surface the composition
/// [`SKELETON`](crate::elements::commission::SKELETON) does not declare **in that
/// tab** — the refusal is about the pair, not the surface alone. Surfaces are
/// code-declared and global, so this leaks nothing and routes answer `422`.
/// Adapters resolve the tab first, so an address wrong in both ways refuses as
/// [`UnknownTab`].
#[derive(Debug)]
pub struct UnknownSurface;

impl std::fmt::Display for UnknownSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no such declared surface")
    }
}

impl std::error::Error for UnknownSurface {}

/// Error source of [`CommissionWrites::remove_element`] when the element does not
/// exist in that commission — an absent id and one belonging to another
/// commission, indistinguishably. Adapters return it so the route can
/// `downcast_ref` and answer `404`. There is no "cannot remove" sibling: every
/// element is removable by construction.
#[derive(Debug)]
pub struct ElementNotFound;

impl std::fmt::Display for ElementNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "element not found in this commission")
    }
}

impl std::error::Error for ElementNotFound {}
