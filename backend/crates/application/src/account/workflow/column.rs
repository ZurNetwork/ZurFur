use macros::WithPorts;

use crate::account::workflow::Workflows;

pub mod add;
pub mod commission;
pub mod remove;
pub mod rename;
pub mod reposition;

#[derive(WithPorts)]
pub struct Columns<'a> {
    #[ports]
    pub workflows: &'a Workflows<'a>,
}
