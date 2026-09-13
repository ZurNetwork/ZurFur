#[derive(Debug)]
pub enum NotFoundEntity {
    Commission,
    Character,
    User,
    Account,
    Element,
}

impl std::fmt::Display for NotFoundEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Account => write!(f, "account"),
            Self::Commission => write!(f, "commission"),
            Self::Character => write!(f, "character"),
            Self::User => write!(f, "user"),
            Self::Element => write!(f, "element"),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum CommonError {
    #[error("The store failed")]
    Infrastructure(#[source] anyhow::Error),
    #[error("{0} not found")]
    NotFound(NotFoundEntity),
    #[error("DID belongs to someone else")]
    DidBelongsToAnotherActor,
    #[error("Handle is unavailable")]
    HandleTaken,
}

impl CommonError {
    pub fn user_not_found() -> Self {
        Self::NotFound(NotFoundEntity::User)
    }
    pub fn character_not_found() -> Self {
        Self::NotFound(NotFoundEntity::Character)
    }
    pub fn account_not_found() -> Self {
        Self::NotFound(NotFoundEntity::Account)
    }
    pub fn element_not_found() -> Self {
        Self::NotFound(NotFoundEntity::Element)
    }
    pub fn commission_not_found() -> Self {
        Self::NotFound(NotFoundEntity::Commission)
    }
}

impl From<anyhow::Error> for CommonError {
    fn from(err: anyhow::Error) -> Self {
        Self::Infrastructure(err)
    }
}
