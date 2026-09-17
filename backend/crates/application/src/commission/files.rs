//! Commission file entries: a Participant uploads a work-in-progress file and
//! a Participant retrieves one. Neither mutates a commission's status.
//!
//! Streaming seam: the port speaks [`tokio::io::AsyncRead`], never a buffered
//! `Vec<u8>` or a driver type.

use macros::WithPorts;

use crate::commission::Commissions;

pub mod download;
pub mod upload;

#[derive(WithPorts)]
pub struct Files<'a> {
    #[ports]
    pub commissions: &'a Commissions<'a>,
}
