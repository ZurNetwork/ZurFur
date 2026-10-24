use crate::ports::WithPorts;

use crate::commission::Commissions;

pub mod clear;
pub mod set;
pub mod status;

#[derive(WithPorts)]
pub struct Deadline<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
