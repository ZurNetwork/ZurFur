use crate::Ports;

pub trait WithPorts<'a> {
    fn ports(&self) -> &'a Ports;
}
