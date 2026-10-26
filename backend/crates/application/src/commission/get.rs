use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, CommissionState, CommissionTitle, DeadlineStatus, DirectionStatus,
            LifecycleStep,
        },
        level::Level,
        maturity::Maturity,
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, Commissions, den_refusal},
    common_error::CommonError,
    den::{
        ActWord, AllowedActs,
        access::{ServedNode, may_open},
        allowed_acts, names,
    },
};

pub struct Query {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

/// A commission as anyone the Den opens it to may see it, and what the viewer may do to it.
/// Never its owner nor its linked channel.
#[derive(Debug)]
pub struct Output {
    pub id: CommissionId,
    /// The title as the Den shows it: `Unnamed` for stored text the name rule refuses.
    pub title: CommissionTitle,
    pub lifecycle: LifecycleStep,
    pub visibility: Level,
    pub deadline: Option<DateTimeUtc>,
    pub maturity: Option<Maturity>,
    pub direction_status: Option<DirectionStatus>,
    /// The deadline-axis status at the read's time: Late derived, else the stored flag.
    pub deadline_status: Option<DeadlineStatus>,
    pub archived: bool,
    /// The creation time at the store's precision.
    pub created_at: DateTimeUtc,
    /// The acts the viewer may take now; empty for someone who holds no role.
    pub acts: Vec<ActWord>,
    /// The steps a lifecycle move would change it to now; empty for someone who holds no role.
    pub lifecycle_targets: Vec<LifecycleStep>,
}

/// A row read after den's check opened it, gone since: the commission's not-found.
fn gone() -> CommissionError {
    CommissionError::NotAMember
}

impl Commissions<'_> {
    /// Reads the commission once den's check opens it to the viewer: its every-opener fields,
    /// its deadline status derived at `now`, and, for a viewer who holds a role, the acts the
    /// den's write would pass. Everyone it doesn't open to gets `NotAMember`, which the drivers
    /// render byte-identically to an absent commission's `404`.
    #[use_case]
    pub async fn get(
        &self,
        #[ports] ports: &Ports,
        query: Query,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Query {
            actor_id,
            commission_id,
        } = query;
        let opens = may_open(ports, &actor_id, ServedNode::Commission(commission_id))
            .await
            .map_err(|refusal| den_refusal(refusal, CommissionError::NotAMember))?;
        let &ServedNode::Commission(commission_id) = opens.node() else {
            return Err(CommissionError::NotAMember);
        };

        let summary = ports
            .commissions
            .find_summary(&commission_id)
            .await?
            .ok_or_else(gone)?;
        let stored = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or_else(gone)?;
        let standing = ports
            .commissions
            .standing(&commission_id, &actor_id)
            .await?
            .ok_or_else(gone)?;
        let shown = names::stored_name(summary.title.as_str()).ok_or_else(|| {
            let unnamed = anyhow::anyhow!("the Den's fallback name breaks the name rule");
            CommonError::Infrastructure(unnamed)
        })?;
        let title = CommissionTitle::from(shown);

        let state = CommissionState::from(&stored);
        let allowed = if standing.role.is_some() {
            allowed_acts(
                ports,
                &actor_id,
                commission_id,
                standing,
                &state,
                &title,
                now,
            )?
        } else {
            AllowedActs::default()
        };

        let output = Output {
            id: commission_id,
            title,
            lifecycle: stored.lifecycle_step.clone(),
            visibility: summary.visibility,
            deadline: stored.deadline,
            maturity: stored.maturity,
            direction_status: stored.direction_status,
            deadline_status: stored.deadline_status_at(now),
            archived: stored.is_archived(),
            created_at: stored.created_at,
            acts: allowed.acts,
            lifecycle_targets: allowed.lifecycle_targets,
        };
        Ok(output)
    }
}
