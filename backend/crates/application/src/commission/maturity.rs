use crate::{commission::Commissions, ports::WithPorts};

pub mod set;

pub struct Maturity<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Maturity<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn maturity(&'a self) -> Maturity<'a> {
        Maturity { commissions: self }
    }
}
