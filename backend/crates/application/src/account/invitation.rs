use macros::WithPorts;

use crate::account::Accounts;

pub mod accept;
pub mod decline;
pub mod issue;
pub mod revoke;

#[derive(WithPorts)]
pub struct Invitations<'a> {
    #[ports]
    accounts: &'a Accounts<'a>,
}
