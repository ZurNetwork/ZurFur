use crate::{commission::Commissions, ports::WithPorts};

pub mod direction;

pub struct Status<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Status<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn status(&'a self) -> Status<'a> {
        Status { commissions: self }
    }
}
