use domain::datetime::DateTimeUtc;

use crate::{commission::Commissions, ports::WithPorts};

pub mod clear;
pub mod set;
pub mod status;

pub(super) struct DeadlineSetEventPayload {
    pub from: Option<String>,
    pub to: Option<String>,
    pub deadline: Option<DateTimeUtc>,
}

pub struct Deadline<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Deadline<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn deadline(&'a self) -> Deadline<'a> {
        Deadline { commissions: self }
    }
}
