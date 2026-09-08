use crate::{account::Accounts, ports::WithPorts};

pub mod accept;
pub mod decline;
pub mod issue;
pub mod revoke;

pub struct Invitations<'a> {
    accounts: &'a Accounts<'a>,
}

impl<'a> WithPorts<'a> for Invitations<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.accounts.ports()
    }
}
impl<'a> Accounts<'a> {
    pub fn invitations(&'a self) -> Invitations<'a> {
        Invitations { accounts: self }
    }
}
