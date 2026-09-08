use crate::{commission::Commissions, ports::WithPorts};

pub mod declare;

pub struct Slots<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Slots<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn slots(&'a self) -> Slots<'a> {
        Slots { commissions: self }
    }
}
