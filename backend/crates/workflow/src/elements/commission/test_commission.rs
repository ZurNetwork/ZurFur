use identity::elements::user::UserId;
use kernel::{datetime::DateTimeUtc, elements::did::Did};

use super::{Commission, CommissionId, DeadlineStatus, LifecycleStep, StoredDispute};

/// A stored commission at `step`, with no deadline and no flags, as a load returns it.
pub(crate) fn at(step: LifecycleStep) -> Commission {
    stored(step, None, None)
}

/// A stored commission at `step`, with `deadline` and the stored `delayed_flag`; at Disputed,
/// disputed from Active since it was made.
pub(crate) fn stored(
    step: LifecycleStep,
    deadline: Option<DateTimeUtc>,
    delayed_flag: Option<DeadlineStatus>,
) -> Commission {
    let created_at = chrono::Utc::now();
    let dispute = (step == LifecycleStep::Disputed).then_some(StoredDispute {
        left: LifecycleStep::Active,
        began: created_at,
    });
    let mut commission = undisputed(step, deadline, delayed_flag, created_at);
    commission.dispute = dispute;
    commission
}

/// A stored commission disputed from `left` since `began`, with `deadline` and no flags.
pub(crate) fn disputed(
    left: LifecycleStep,
    began: DateTimeUtc,
    deadline: Option<DateTimeUtc>,
) -> Commission {
    let mut commission = undisputed(LifecycleStep::Disputed, deadline, None, began);
    commission.dispute = Some(StoredDispute { left, began });
    commission
}

/// A stored commission at `step` with no dispute, made at `created_at`.
fn undisputed(
    step: LifecycleStep,
    deadline: Option<DateTimeUtc>,
    delayed_flag: Option<DeadlineStatus>,
    created_at: DateTimeUtc,
) -> Commission {
    let owner = UserId::from(Did::from("did:plc:owner".to_owned()));
    Commission {
        id: CommissionId::mint(),
        owner_id: owner,
        lifecycle_step: step,
        dispute: None,
        deadline,
        maturity: None,
        direction_status: None,
        delayed_flag,
        linked_channel: None,
        archived_at: None,
        created_at,
    }
}
