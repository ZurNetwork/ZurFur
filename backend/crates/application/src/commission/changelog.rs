use crate::{commission::Commissions, ports::WithPorts};

pub mod read;

pub struct Changelog<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Changelog<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn changelog(&'a self) -> Changelog<'a> {
        Changelog { commissions: self }
    }
}
