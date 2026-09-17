use macros::WithPorts;

use crate::commission::Commissions;

pub mod set;

#[derive(WithPorts)]
pub struct Maturity<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
