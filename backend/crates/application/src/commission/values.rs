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
    pub deadline_status: Option<DeadlineStatus>,
    pub linked_channel: Option<ChannelPointer>,
    /// The creation time at the store's precision.
    pub created_at: DateTimeUtc,
}

impl CommissionValues {
    /// The loaded commission's values, named `title` and levelled `visibility` as its node
    /// holds them. Not `From`: the commission, its name and its level are three facts.
    pub(crate) fn from_parts(commission: Commission, title: StoredText, visibility: Level) -> Self {
        Self {
            id: commission.id,
            title,
            lifecycle: commission.lifecycle_step,
            visibility,
            deadline: commission.deadline,
            maturity: commission.maturity,
            direction_status: commission.direction_status,
            deadline_status: commission.deadline_status,
            linked_channel: commission.linked_channel,
            created_at: commission.created_at,
        }
    }
}

impl TryFrom<WriteResult> for CommissionValues {
    type Error = CommissionError;

    /// The commission the Den's write answered, its id the thing written, named and levelled
    /// as the write left its node.
    fn try_from(written: WriteResult) -> Result<Self, Self::Error> {
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
            ..Self::from_parts(commission, title, level)
        };
        Ok(values)
    }
}
