use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, CommissionRole, NewChangelogEntry},
        user::UserId,
    },
    string_builder::StringBuilder,
};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, notes::Notes, require},
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub content: String,
}
pub struct Output;
impl Notes<'_> {
    #[use_case]
    pub async fn attach(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            content,
        } = cmd;
        require(
            ports,
            &commission_id,
            &actor_id,
            CommissionRole::Participant,
        )
        .await?;

        let text = StringBuilder::new(content)
            .trimmed()
            .non_empty()
            .build()
            .map_err(|_| CommissionError::IncorrectContent)?;

        let entry = NewChangelogEntry::note(commission_id, actor_id, text, now);
        let uow = uow.open().await?;
        uow.changelog().append(&entry).await?;
        Ok(Output)
    }
}
