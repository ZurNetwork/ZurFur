use crate::{commission::Commissions, ports::WithPorts};

pub mod declare;

pub struct Seats<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Seats<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn seats(&'a self) -> Seats<'a> {
        Seats { commissions: self }
    }
}
