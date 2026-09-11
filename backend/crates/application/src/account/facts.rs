use crate::{account::Accounts, ports::WithPorts};

pub mod exist;

pub struct Facts<'a> {
    accounts: &'a Accounts<'a>,
}

impl<'a> WithPorts<'a> for Facts<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.accounts.ports()
    }
}

impl<'a> Accounts<'a> {
    pub fn facts(&'a self) -> Facts<'a> {
        Facts { accounts: self }
    }
}
