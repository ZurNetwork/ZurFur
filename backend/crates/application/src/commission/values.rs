use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChannelPointer, Commission, CommissionId, DeadlineStatus, DirectionStatus,
            LifecycleStep,
        },
        level::Level,
        maturity::Maturity,
        text::StoredText,
    },
};

use crate::{
    commission::CommissionError,
    common_error::CommonError,
    den::{DenThing, act::ArmAnswer, write::WriteResult},
};

/// A commission's values after a write: what create and rename answer.
#[derive(Debug)]
pub struct CommissionValues {
    pub id: CommissionId,
    /// The title as stored, after the Den's name rule stripped what it strips.
    pub title: StoredText,
    pub lifecycle: LifecycleStep,
    pub visibility: Level,
    pub deadline: Option<DateTimeUtc>,
    pub maturity: Option<Maturity>,
    pub direction_status: Option<DirectionStatus>,
    /// The deadline-axis status at the act's time: Late derived, else the stored flag.
    pub deadline_status: Option<DeadlineStatus>,
    pub linked_channel: Option<ChannelPointer>,
    /// The creation time at the store's precision.
    pub created_at: DateTimeUtc,
}

impl CommissionValues {
    /// The loaded commission's values, named `title` and levelled `visibility` as its node
    /// holds them, its deadline status derived at `now`.
    fn from_parts(
        commission: Commission,
        title: StoredText,
        visibility: Level,
        now: DateTimeUtc,
    ) -> Self {
        let deadline_status = commission.deadline_status_at(now);
        Self {
            id: commission.id,
            title,
            lifecycle: commission.lifecycle_step,
            visibility,
            deadline: commission.deadline,
            maturity: commission.maturity,
            direction_status: commission.direction_status,
            deadline_status,
            linked_channel: commission.linked_channel,
            created_at: commission.created_at,
        }
    }

    /// The commission the Den's write answered, its id the thing written, named and levelled
    /// as the write left its node, its deadline status derived at the act's time `now`. Not
    /// `TryFrom`: the write's answer and the act's time are two facts.
    pub(crate) fn answered(
        written: WriteResult,
        now: DateTimeUtc,
    ) -> Result<Self, CommissionError> {
        let WriteResult {
            thing,
            name,
            level,
            answer: ArmAnswer::Commission(commission),
        } = written;
        let DenThing::Commission(id) = thing else {
            let other = anyhow::anyhow!("the den's write answered no commission");
            return Err(CommonError::Infrastructure(other).into());
        };
        let name = name.ok_or_else(|| {
            let unnamed = anyhow::anyhow!("the den's write gave the commission no name");
            CommonError::Infrastructure(unnamed)
        })?;
        let title = StoredText::from(name.as_ref().to_owned());
        let values = Self {
            id,
            ..Self::from_parts(commission, title, level, now)
        };
        Ok(values)
    }
}
