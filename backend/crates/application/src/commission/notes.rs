use crate::ports::WithPorts;

use crate::commission::Commissions;

pub mod attach;

#[derive(WithPorts)]
pub struct Notes<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
