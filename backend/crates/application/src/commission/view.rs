use crate::commission::Commissions;
use crate::ports::WithPorts;
pub mod grant;
pub mod revoke;

#[derive(WithPorts)]
pub struct View<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
