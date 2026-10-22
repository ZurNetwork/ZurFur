use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, Commission, CommissionTitle, NewChangelogEntry},
        maturity::Maturity,
        user::UserId,
    },
    ports::Unit,
};
use serde_json::json;

use crate::{
    commission::{CommissionResult, CommissionValues, Commissions, den_title},
    common_error::CommonError,
};

pub struct Command {
    pub actor_id: UserId,
    pub title: CommissionTitle,
    pub maturity: Option<Maturity>,
    pub deadline: Option<DateTimeUtc>,
}

impl Commissions<'_> {
    /// Create a commission owned by the caller, with its `created` entry, and answer the row
    /// as the store holds it. The entry names no title.
    #[use_case]
    pub async fn create(
        &self,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<CommissionValues> {
        let Command {
            actor_id,
            title,
            maturity,
            deadline,
        } = cmd;
        let title = den_title(&title)?;

        let mut commission = Commission::create(title, actor_id.clone(), now, deadline);
        commission.maturity = maturity;
        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Created,
            actor_id,
            json!({}),
            now,
        );

        #[expect(
            clippy::disallowed_methods,
            reason = "the ungated create site, frozen until its slice routes it through the den's write gate"
        )]
        uow.commissions().create(&commission).await?;
        uow.changelog().append(&entry).await?;

        let stored = uow
            .commissions()
            .find(&commission.id)
            .await?
            .ok_or_else(|| {
                let unread = anyhow::anyhow!("the created commission could not be read back");
                CommonError::Infrastructure(unread)
            })?;
        Ok(CommissionValues::from(stored))
    }
}

#[cfg(test)]
mod tests;
