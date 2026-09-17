use macros::WithPorts;

use crate::commission::Commissions;

pub mod issue;
pub mod revoke;

#[derive(WithPorts)]
pub struct Invitations<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
