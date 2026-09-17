use macros::WithPorts;

use crate::commission::Commissions;

pub mod read;

#[derive(WithPorts)]
pub struct Changelog<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
