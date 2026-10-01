use crate::ports::WithPorts;
use domain::datetime::DateTimeUtc;

use crate::commission::Commissions;

pub mod clear;
pub mod set;
pub mod status;

pub(super) struct DeadlineSetEventPayload {
    pub from: Option<String>,
    pub to: Option<String>,
    pub deadline: Option<DateTimeUtc>,
}

#[derive(WithPorts)]
pub struct Deadline<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
