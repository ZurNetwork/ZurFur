pub mod create;
pub mod delete;

use crate::{common_error::CommonError, ports::WithPorts};

pub struct Characters<'a> {
    pub ports: &'a crate::Ports,
}

impl<'a> WithPorts<'a> for Characters<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.ports
    }
}
impl<'a> Characters<'a> {
    /// Bind the namespace to resolved dependencies.
    pub fn new(ports: &'a crate::Ports) -> Self {
        Self { ports }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CharacterError {
    #[error(transparent)]
    Common(#[from] CommonError),
}

impl From<anyhow::Error> for CharacterError {
    fn from(err: anyhow::Error) -> Self {
        // FIXME: This needs more stuff
        Self::Common(err.into())
    }
}

pub type CharacterResult<T> = Result<T, CharacterError>;
