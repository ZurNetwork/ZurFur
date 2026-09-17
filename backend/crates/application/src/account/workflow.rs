use macros::WithPorts;

use crate::account::Accounts;

pub mod column;
pub mod create;
pub mod delete;

#[derive(WithPorts)]
pub struct Workflows<'a> {
    #[ports]
    pub accounts: &'a Accounts<'a>,
}
