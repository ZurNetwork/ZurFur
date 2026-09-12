use crate::{commission::Commissions, ports::WithPorts};

pub mod attach;

pub struct Notes<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Notes<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn notes(&'a self) -> Notes<'a> {
        Notes { commissions: self }
    }
}
