use crate::elements::did::Did;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(uuid::Uuid);

pub struct User {
    pub id: UserId,
    pub did: Did,
}
