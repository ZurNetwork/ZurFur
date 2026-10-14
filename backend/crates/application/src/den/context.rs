use domain::elements::user::UserId;

use crate::Ports;

/// How a read names the viewer's own root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootNaming {
    /// From the profile cache, else through one lookup of the viewer's own DID.
    LookUp,
    /// From the profile cache, else by the DID: no lookup started, no wait.
    CacheOnly,
}

/// What every arm of the host is handed: the ports, and the viewer the session names.
#[derive(Clone, Copy)]
pub(crate) struct DenContext<'a> {
    /// The ports an arm reads through.
    pub(crate) ports: &'a Ports,
    /// The viewer, from the session only.
    pub(crate) viewer: &'a UserId,
    /// How the viewer's root is named on this read.
    pub(crate) root_naming: RootNaming,
}
