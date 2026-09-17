use macros::WithPorts;

use crate::commission::status::Status;

pub mod clear;
pub mod set;

#[derive(WithPorts)]
pub struct Direction<'a> {
    #[ports]
    status: &'a Status<'a>,
}
