/// Error source of an element write whose tab does not exist in that commission
/// — an absent tab id and one belonging to another commission, indistinguishably,
/// so probing tab ids reveals nothing. Adapters return it so the route can
/// `downcast_ref` and answer `404`.
#[derive(Debug, thiserror::Error)]
#[error("tab not found in this commission")]
pub struct UnknownTab;

/// Error source of an element write whose surface the composition
/// [`SKELETON`](workflow::elements::commission::SKELETON) does not declare **in that
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
