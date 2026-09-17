use macros::WithPorts;

use crate::account::Accounts;

pub mod grant;
pub mod revoke;

#[derive(WithPorts)]
pub struct Roles<'a> {
    #[ports]
    accounts: &'a Accounts<'a>,
}
