//! Commission file entries (ZMVP-88; moved down from `api`, ZMVP-205): a
//! Participant uploads a work-in-progress file, and a Participant retrieves
//! one. Neither ever mutates a commission's status.
//!
//! **Streaming seam (Engineer ruling 2026-08-31).** The port speaks
//! [`tokio::io::AsyncRead`], never a buffered `Vec<u8>` or an axum type —
//! `api` adapts multipart to a reader on the way in and a byte stream on the
//! way out; a CLI could hand this a `tokio::fs::File` just as well.

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
