pub mod claim;
pub mod create;
pub mod delete;
pub mod transfer;
pub mod visibility;

use macros::WithPorts;

use crate::common_error::CommonError;

#[derive(WithPorts)]
pub struct Characters<'a> {
    #[ports(is_root = true)]
    pub ports: &'a crate::Ports,
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
    #[error("Incorrect role")]
    IncorrectRole,
}

impl From<anyhow::Error> for CharacterError {
    fn from(err: anyhow::Error) -> Self {
        // FIXME: This needs more stuff
        Self::Common(err.into())
    }
}

pub type CharacterResult<T> = Result<T, CharacterError>;
