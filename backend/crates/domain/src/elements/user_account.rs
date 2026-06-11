use crate::elements::{account::AccountId, role::Role, user::UserId};

pub struct UserAccount(pub UserId, pub AccountId, pub Role);
