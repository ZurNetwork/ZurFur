use std::ops::Deref;

use crate::{datetime::DateTimeUtc, elements::did::Did};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(uuid::Uuid);

impl Deref for AccountId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Account {
    pub id: AccountId,
    pub did: Did,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
}

pub struct AccountProfile {
    pub did: Did,
    pub display_name: String,
}
