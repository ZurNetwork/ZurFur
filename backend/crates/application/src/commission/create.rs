use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, CommissionTitle, NewChangelogEntry,
        },
        maturity::Maturity,
        user::UserId,
    },
};
use serde_json::json;

use crate::commission::{CommissionResult, Commissions};

pub struct Command {
    pub actor_id: UserId,
    pub title: CommissionTitle,
    pub maturity: Option<Maturity>,
    pub deadline: Option<DateTimeUtc>,
}
pub struct Output {
    pub id: CommissionId,
}

impl Commissions<'_> {
    pub async fn create(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let Command {
            actor_id,
            title,
            maturity,
            deadline,
        } = cmd;

        let mut commission = Commission::create(title, actor_id.clone(), now, deadline);
        commission.maturity = maturity;
        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Created,
            actor_id,
            json!({ "title": commission.title.as_str() }),
            now,
        );

        let mut uow = self.ports().database.begin().await?;
        uow.commissions().create(&commission).await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;

        Ok(Output { id: commission.id })
    }
}
