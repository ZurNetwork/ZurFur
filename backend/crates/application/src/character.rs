pub mod create;
pub mod delete;
pub(crate) mod den;

use crate::ports::WithPorts;

use crate::{common_error::CommonError, den::DenNameError};

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
    /// The name breaks the Den's name rule.
    #[error("the character's name is refused")]
    Name(#[from] DenNameError),
}

impl From<anyhow::Error> for CharacterError {
    fn from(err: anyhow::Error) -> Self {
        // FIXME: This needs more stuff
        Self::Common(err.into())
    }
}

pub type CharacterResult<T> = Result<T, CharacterError>;
