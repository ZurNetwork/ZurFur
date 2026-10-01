use domain::elements::account::AccountId;

use crate::account::{AccountResult, facts::Facts};
use crate::use_case;

pub struct Query {
    pub account_id: AccountId,
}
pub struct Output {
    pub has_facts: bool,
}

impl<'a> Facts<'a> {
    #[use_case]
    pub async fn exist(&self, _query: Query) -> AccountResult<Output> {
        Ok(Output { has_facts: false })
    }
}
