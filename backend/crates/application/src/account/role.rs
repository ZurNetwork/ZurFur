use crate::{account::Accounts, ports::WithPorts};

pub mod grant;
pub mod revoke;
pub struct Roles<'a> {
    accounts: &'a Accounts<'a>,
}

impl<'a> WithPorts<'a> for Roles<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.accounts.ports()
    }
}
impl<'a> Accounts<'a> {
    pub fn roles(&'a self) -> Roles<'a> {
        Roles { accounts: self }
    }
}
