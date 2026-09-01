use crate::{account::Accounts, ports::WithPorts};

pub mod column;
pub mod create;
pub mod delete;

pub struct Workflows<'a> {
    pub accounts: &'a Accounts<'a>,
}

impl<'a> WithPorts<'a> for Workflows<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.accounts.ports()
    }
}

impl<'a> Accounts<'a> {
    pub fn workflows(&'a self) -> Workflows<'a> {
        Workflows { accounts: self }
    }
}
