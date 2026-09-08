use crate::{commission::Commissions, ports::WithPorts};

pub mod issue;
pub mod revoke;

pub struct Invitations<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Invitations<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn invitations(&'a self) -> Invitations<'a> {
        Invitations { commissions: self }
    }
}
