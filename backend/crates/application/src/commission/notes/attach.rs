use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, NewChangelogEntry},
        user::UserId,
    },
    string_builder::StringBuilder,
};

use crate::{
    commission::{CommissionError, CommissionResult, notes::Notes},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub content: String,
}
pub struct Output;
impl Notes<'_> {
    pub async fn attach(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            content,
        } = cmd;
        if !ports
            .commissions
            .is_participant(&commission_id, &actor_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        };

        let text = StringBuilder::new(content)
            .trimmed()
            .non_empty()
            .build()
            .map_err(|_| CommissionError::IncorrectContent)?;

        let entry = NewChangelogEntry::note(commission_id, actor_id, text, now);
        let mut uow = self.ports().database.begin().await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
