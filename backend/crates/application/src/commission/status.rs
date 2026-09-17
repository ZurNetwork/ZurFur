use crate::commission::Commissions;
use macros::WithPorts;

pub mod direction;

#[derive(WithPorts)]
pub struct Status<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
