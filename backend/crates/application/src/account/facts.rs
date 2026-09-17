use macros::WithPorts;

use crate::account::Accounts;

pub mod exist;

#[derive(WithPorts)]
pub struct Facts<'a> {
    #[ports]
    accounts: &'a Accounts<'a>,
}
