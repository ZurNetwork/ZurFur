use crate::{commission::status::Status, ports::WithPorts};

pub mod clear;
pub mod set;

pub struct Direction<'a> {
    status: &'a Status<'a>,
}

impl<'a> WithPorts<'a> for Direction<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.status.ports()
    }
}

impl<'a> Status<'a> {
    pub fn direction(&'a self) -> Direction<'a> {
        Direction { status: self }
    }
}
