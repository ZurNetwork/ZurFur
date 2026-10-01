use crate::Ports;

// The derive and the trait share a name in separate namespaces (the serde idiom).
pub use application_macros::WithPorts;

pub trait WithPorts<'a> {
    fn ports(&self) -> &'a Ports;
}
