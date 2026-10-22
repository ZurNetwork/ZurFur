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

/// A commission's values as the store holds them, read back after a write: what create and
/// rename answer.
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

impl From<Commission> for CommissionValues {
    /// The loaded commission's values, field for field.
    fn from(commission: Commission) -> Self {
        Self {
            id: commission.id,
            title: commission.title,
            lifecycle: commission.lifecycle_step,
            visibility: commission.visibility,
            deadline: commission.deadline,
            maturity: commission.maturity,
            direction_status: commission.direction_status,
            deadline_status: commission.deadline_status,
            linked_channel: commission.linked_channel,
            created_at: commission.created_at,
        }
    }
}
