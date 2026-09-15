#[derive(Debug, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum NotFoundEntity {
    Commission,
    Character,
    User,
    Account,
    Element,
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

#[cfg(test)]
mod tests;
