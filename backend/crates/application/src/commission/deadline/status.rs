use crate::ports::WithPorts;

use crate::commission::deadline::Deadline;

pub mod clear;
pub mod set;

#[derive(WithPorts)]
pub struct Status<'a> {
    #[ports]
    pub deadline: &'a Deadline<'a>,
}
