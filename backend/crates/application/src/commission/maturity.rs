//! A commission's rating: given and replaced by its owner through the Den's write.

use crate::ports::WithPorts;

use crate::commission::Commissions;

pub mod set;

/// Rating use cases, under [`Commissions`].
#[derive(WithPorts)]
pub struct Maturity<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
