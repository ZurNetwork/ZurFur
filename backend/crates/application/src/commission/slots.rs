use macros::WithPorts;

use crate::commission::Commissions;

pub mod declare;

#[derive(WithPorts)]
pub struct Slots<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
