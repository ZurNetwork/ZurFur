use crate::{account::workflow::Workflows, ports::WithPorts};

pub mod add;
pub mod commission;
pub mod remove;
pub mod rename;
pub mod reposition;

pub struct Columns<'a> {
    pub workflows: &'a Workflows<'a>,
}

impl<'a> WithPorts<'a> for Columns<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.workflows.ports()
    }
}

impl<'a> Workflows<'a> {
    pub fn columns(&'a self) -> Columns<'a> {
        Columns { workflows: self }
    }
}
