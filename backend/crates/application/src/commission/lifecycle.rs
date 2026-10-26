//! A commission's lifecycle step: moved by a Participant through the Den's write.

use crate::ports::WithPorts;

use crate::commission::Commissions;

pub mod move_to;

/// Lifecycle use cases, under [`Commissions`].
#[derive(WithPorts)]
pub struct Lifecycle<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
