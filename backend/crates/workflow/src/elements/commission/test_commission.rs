use identity::elements::user::UserId;
use kernel::{datetime::DateTimeUtc, elements::did::Did};

use super::{Commission, CommissionId, DeadlineStatus, LifecycleStep};

/// A stored commission at `step`, with no deadline and no flags, as a load returns it.
pub(crate) fn at(step: LifecycleStep) -> Commission {
    stored(step, None, None)
}

/// A stored commission at `step`, with `deadline` and the stored `delayed_flag`.
pub(crate) fn stored(
    step: LifecycleStep,
    deadline: Option<DateTimeUtc>,
    delayed_flag: Option<DeadlineStatus>,
) -> Commission {
    let owner = UserId::from(Did::from("did:plc:owner".to_owned()));
    Commission {
        id: CommissionId::mint(),
        owner_id: owner,
        lifecycle_step: step,
        deadline,
        maturity: None,
        direction_status: None,
        delayed_flag,
        linked_channel: None,
        archived_at: None,
        created_at: chrono::Utc::now(),
    }
}
