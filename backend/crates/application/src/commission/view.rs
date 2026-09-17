use crate::commission::Commissions;
use macros::WithPorts;
pub mod grant;
pub mod revoke;

#[derive(WithPorts)]
pub struct View<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
