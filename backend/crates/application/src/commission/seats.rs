use crate::ports::WithPorts;

use crate::commission::Commissions;

pub mod declare;

#[derive(WithPorts)]
pub struct Seats<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
