//! A commission's dispute: raised and cleared by a Participant through the Den's write.

use crate::ports::WithPorts;

use crate::commission::Commissions;

pub mod clear;
pub mod raise;

/// Dispute use cases, under [`Commissions`].
#[derive(WithPorts)]
pub struct Dispute<'a> {
    #[ports]
    commissions: &'a Commissions<'a>,
}
