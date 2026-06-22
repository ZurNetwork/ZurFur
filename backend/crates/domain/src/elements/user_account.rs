use crate::elements::{account::AccountId, role::Role, user::UserId};

pub struct UserAccount(pub UserId, pub AccountId, pub Role);

impl UserAccount {
    pub fn get_user_id(&self) -> UserId {
        self.0
    }

    pub fn get_account_id(&self) -> AccountId {
        self.1
    }

    pub fn get_role(&self) -> Role {
        self.2.clone()
    }
}
