use crate::{commission::Commissions, ports::WithPorts};

pub mod grant;
pub mod revoke;

pub struct View<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for View<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn view(&'a self) -> View<'a> {
        View { commissions: self }
    }
}
