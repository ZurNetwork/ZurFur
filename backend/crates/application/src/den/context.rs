use domain::elements::user::UserId;

use crate::Ports;

/// What every arm of the host is handed: the ports, and the viewer the session names.
#[derive(Clone, Copy)]
pub(crate) struct DenContext<'a> {
    /// The ports an arm reads through.
    #[allow(
        dead_code,
        reason = "read by the identity and commission arms once they serve nodes"
    )]
    pub(crate) ports: &'a Ports,
    /// The viewer, from the session only.
    pub(crate) viewer: &'a UserId,
}
