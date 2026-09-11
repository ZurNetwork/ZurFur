//! Commission file entries: a Participant uploads a work-in-progress file and
//! a Participant retrieves one. Neither mutates a commission's status.
//!
//! Streaming seam: the port speaks [`tokio::io::AsyncRead`], never a buffered
//! `Vec<u8>` or a driver type.

use crate::{commission::Commissions, ports::WithPorts};

pub mod download;
pub mod upload;

pub struct Files<'a> {
    pub commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Files<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn files(&'a self) -> Files<'a> {
        Files { commissions: self }
    }
}
