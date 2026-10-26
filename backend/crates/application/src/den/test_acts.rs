//! Test-only acts: the real create, rename, dispute, field, rating and Late acts as the den carries them, the
//! system as an actor, each field act's typed use case called by one value, a commission's
//! stored state at any step and with any field (and its entries and fields read back), and
//! Atta's acts built by hand around a real act's body, for the tests that drive the write host
//! below the den's builder.

use atta::{Act, ActName, Born, Placed, RealPath, Segment, Shape};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            Commission, CommissionDeadline, CommissionId, CommissionState, DeadlineStatus,
            DirectionStatus, LifecycleStep, NewCommission, NewSlot, SlotDeclaration, StoredDispute,
            SurfaceAddress, TabId,
        },
        did::Did,
        maturity::Maturity,
        name::NodeName,
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        CommissionError, Commissions, act::CommissionAct, create, deadline, den::CommissionPlace,
        dispute, lifecycle::move_to, maturity, rename, slots::declare, status::direction, sweep,
    },
    den::{DenType, Place, act::DenAct, host::type_key, names, write::commission_directory},
};

/// A name from test text.
fn name(text: &str) -> NodeName {
    NodeName::try_from(text).expect("a valid test name")
}

/// The create act for a commission titled `text`, choosing no maturity and no deadline.
pub(crate) fn create_act(text: &str) -> DenAct {
    let act = create::Act {
        title: text.parse().expect("a valid test title"),
        chosen: NewCommission {
            maturity: None,
            deadline: None,
        },
    };
    DenAct::Commission(CommissionAct::Create(act))
}

/// The rename act giving the commission `commission` the name `text`.
pub(crate) fn rename_act(commission: CommissionId, text: &str) -> DenAct {
    let act = rename::Act {
        commission,
        title: text.parse().expect("a valid test title"),
    };
    DenAct::Commission(CommissionAct::Rename(act))
}

/// The move of the commission `commission` to `to`.
pub(crate) fn move_act(commission: CommissionId, to: LifecycleStep) -> DenAct {
    let act = move_to::Act { commission, to };
    DenAct::Commission(CommissionAct::MoveLifecycle(act))
}

/// The dispute raised on the commission `commission`.
pub(crate) fn raise_dispute_act(commission: CommissionId) -> DenAct {
    let act = dispute::raise::Act { commission };
    DenAct::Commission(CommissionAct::RaiseDispute(act))
}

/// The dispute on the commission `commission` cleared.
pub(crate) fn clear_dispute_act(commission: CommissionId) -> DenAct {
    let act = dispute::clear::Act { commission };
    DenAct::Commission(CommissionAct::ClearDispute(act))
}

/// The direction set of the commission `commission` to `to`.
pub(crate) fn set_direction_act(commission: CommissionId, to: DirectionStatus) -> DenAct {
    let act = direction::set::Act { commission, to };
    DenAct::Commission(CommissionAct::SetDirection(act))
}

/// The direction clear of the commission `commission`.
pub(crate) fn clear_direction_act(commission: CommissionId) -> DenAct {
    let act = direction::clear::Act { commission };
    DenAct::Commission(CommissionAct::ClearDirection(act))
}

/// The deadline set of the commission `commission` to `deadline`.
pub(crate) fn set_deadline_act(commission: CommissionId, deadline: DateTimeUtc) -> DenAct {
    let act = deadline::set::Act {
        commission,
        deadline: CommissionDeadline::from(deadline),
    };
    DenAct::Commission(CommissionAct::SetDeadline(act))
}

/// The deadline clear of the commission `commission`.
pub(crate) fn clear_deadline_act(commission: CommissionId) -> DenAct {
    let act = deadline::clear::Act { commission };
    DenAct::Commission(CommissionAct::ClearDeadline(act))
}

/// The Delayed flag raised on the commission `commission`.
pub(crate) fn set_delayed_act(commission: CommissionId) -> DenAct {
    let act = deadline::status::set::Act { commission };
    DenAct::Commission(CommissionAct::SetDelayed(act))
}

/// The Delayed flag taken down on the commission `commission`.
pub(crate) fn clear_delayed_act(commission: CommissionId) -> DenAct {
    let act = deadline::status::clear::Act { commission };
    DenAct::Commission(CommissionAct::ClearDelayed(act))
}

/// The rating of the commission `commission` to `to`.
pub(crate) fn set_maturity_act(commission: CommissionId, to: Maturity) -> DenAct {
    let act = maturity::set::Act { commission, to };
    DenAct::Commission(CommissionAct::SetMaturity(act))
}

/// The system's Late mark on the commission `commission`.
pub(crate) fn mark_late_act(commission: CommissionId) -> DenAct {
    let act = sweep::Act { commission };
    DenAct::Commission(CommissionAct::MarkLate(act))
}

/// The declare of one Slot titled `text` on `commission`, its element on `tab`'s `content`
/// surface, under a freshly minted element id.
pub(crate) fn declare_act(commission: CommissionId, text: &str, tab: TabId) -> DenAct {
    let slot = NewSlot {
        address: SurfaceAddress::new(tab, "content".parse().expect("a surface name")),
        notes: None,
    };
    let act = declare::Act {
        commission,
        title: text.parse().expect("a valid test title"),
        declaration: SlotDeclaration::minted(slot),
    };
    DenAct::Commission(CommissionAct::DeclareSlot(act))
}

/// The system, acting: what only the sweep may name.
pub(crate) fn the_system() -> crate::den::write::Actor {
    crate::den::write::Actor::System(sweep::SweepToken::for_test())
}

/// One call of a field act's typed use case, by what it carries.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FieldCall {
    /// Raises a dispute.
    RaiseDispute,
    /// Clears the dispute.
    ClearDispute,
    /// Sets the direction status.
    SetDirection(DirectionStatus),
    /// Clears the direction status.
    ClearDirection,
    /// Sets or moves the deadline.
    SetDeadline(DateTimeUtc),
    /// Clears the deadline.
    ClearDeadline,
    /// Raises the Delayed flag.
    SetDelayed,
    /// Takes the Delayed flag down.
    ClearDelayed,
    /// Rates the commission, an Owner's act.
    SetMaturity(Maturity),
}

/// `actor`'s `call` on `commission` at `now`, through its typed use case.
pub(crate) async fn field_call(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    call: FieldCall,
    now: DateTimeUtc,
) -> Result<(), CommissionError> {
    let actor_id = actor.clone();
    let commissions = Commissions::new(ports);
    match call {
        FieldCall::RaiseDispute => {
            let command = dispute::raise::Command {
                actor_id,
                commission_id: commission,
            };
            commissions.dispute().raise(command, now).await?;
        }
        FieldCall::ClearDispute => {
            let command = dispute::clear::Command {
                actor_id,
                commission_id: commission,
            };
            commissions.dispute().clear(command, now).await?;
        }
        FieldCall::SetDirection(to) => {
            let command = direction::set::Command {
                actor_id,
                commission_id: commission,
                to,
            };
            commissions.status().direction().set(command, now).await?;
        }
        FieldCall::ClearDirection => {
            let command = direction::clear::Command {
                actor_id,
                commission_id: commission,
            };
            commissions.status().direction().clear(command, now).await?;
        }
        FieldCall::SetDeadline(deadline) => {
            let command = deadline::set::Command {
                actor_id,
                commission_id: commission,
                deadline,
            };
            commissions.deadline().set(command, now).await?;
        }
        FieldCall::ClearDeadline => {
            let command = deadline::clear::Command {
                actor_id,
                commission_id: commission,
            };
            commissions.deadline().clear(command, now).await?;
        }
        FieldCall::SetDelayed => {
            let command = deadline::status::set::Command {
                actor_id,
                commission_id: commission,
            };
            commissions.deadline().status().set(command, now).await?;
        }
        FieldCall::ClearDelayed => {
            let command = deadline::status::clear::Command {
                actor_id,
                commission_id: commission,
            };
            commissions.deadline().status().clear(command, now).await?;
        }
        FieldCall::SetMaturity(to) => {
            let command = maturity::set::Command {
                actor_id,
                commission_id: commission,
                maturity_rating: to.rating,
                graphic: to.graphic,
            };
            commissions.maturity().run(command, now).await?;
        }
    }
    Ok(())
}

/// Every changelog entry of `commission`.
pub(crate) async fn entries_of(
    ports: &Ports,
    commission: CommissionId,
) -> Vec<domain::elements::commission::ChangelogEntry> {
    ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
}

/// The stored fields of `commission`, as a rule reads them.
pub(crate) async fn fields_of(ports: &Ports, commission: CommissionId) -> CommissionState {
    let stored = ports
        .commission_rows
        .find(&commission)
        .await
        .expect("the store reads")
        .expect("the commission is stored");
    CommissionState::from(&stored)
}

/// A stored commission's state at `step`, with no deadline, flag, status or rating.
pub(crate) fn state_at(step: LifecycleStep) -> CommissionState {
    state_with(step, None, None, None)
}

/// A stored commission's state at `step`, with `deadline`, the stored Delayed `flag` and the
/// direction `status`, and no rating; at Disputed, disputed from Active since it was made.
pub(crate) fn state_with(
    step: LifecycleStep,
    deadline: Option<DateTimeUtc>,
    flag: Option<DeadlineStatus>,
    status: Option<DirectionStatus>,
) -> CommissionState {
    let created_at = chrono::Utc::now();
    let dispute = (step == LifecycleStep::Disputed).then_some(StoredDispute {
        left: LifecycleStep::Active,
        began: created_at,
    });
    let commission = Commission {
        id: CommissionId::mint(),
        owner_id: UserId::from(Did::from("did:plc:state-owner".to_owned())),
        lifecycle_step: step,
        dispute,
        deadline,
        maturity: None,
        direction_status: status,
        delayed_flag: flag,
        linked_channel: None,
        archived_at: None,
        created_at,
    };
    CommissionState::from(&commission)
}

/// A stored commission's state disputed from `left`, with no deadline, flag, status or rating.
pub(crate) fn state_disputed_from(left: LifecycleStep) -> CommissionState {
    let commission = Commission {
        id: CommissionId::mint(),
        owner_id: UserId::from(Did::from("did:plc:state-owner".to_owned())),
        lifecycle_step: LifecycleStep::Disputed,
        dispute: Some(StoredDispute {
            left,
            began: chrono::Utc::now(),
        }),
        deadline: None,
        maturity: None,
        direction_status: None,
        delayed_flag: None,
        linked_channel: None,
        archived_at: None,
        created_at: chrono::Utc::now(),
    };
    CommissionState::from(&commission)
}

/// The real path of the commission `commission`.
pub(crate) fn commission_path(commission: CommissionId) -> RealPath {
    Place::Commission(CommissionPlace::Commission(commission))
        .anchored_path()
        .expect("a commission has a path")
}

/// Atta's act built by hand around `body`, renaming the Private node of `den_type` at `path`
/// to `text`.
pub(crate) fn hand_built_rename_of(
    path: RealPath,
    den_type: DenType,
    text: &str,
    body: DenAct,
) -> Act<DenAct> {
    let typ = type_key(den_type);
    let node = Placed {
        path,
        typ: typ.clone(),
        level: atta::Level::Private,
    };
    let shape = Shape::Rename {
        node,
        name: names::display_name(&name(text)),
    };
    Act {
        name: ActName::from("rename".to_owned()),
        typ,
        shape,
        body,
    }
}

/// Atta's act built by hand around the real rename of `commission` to `text`, renaming the
/// commission at `path`.
pub(crate) fn hand_built_rename(
    path: RealPath,
    commission: CommissionId,
    text: &str,
) -> Act<DenAct> {
    let body = rename_act(commission, text);
    hand_built_rename_of(path, DenType::Commission, text, body)
}

/// Atta's act built by hand around the real create of a commission titled `text`, creating
/// it keyed `commission` in `/commission`, Private, holding no entries.
pub(crate) fn hand_built_create(commission: CommissionId, text: &str) -> Act<DenAct> {
    let body = create_act(text);
    hand_built_create_of(commission, text, DenType::Commission, None, body)
}

/// Atta's act built by hand around `body`, creating a node of `den_type` keyed `commission` in
/// `/commission`, named `text`, at the level `picked`, holding no entries.
pub(crate) fn hand_built_create_of(
    commission: CommissionId,
    text: &str,
    den_type: DenType,
    picked: Option<atta::Level>,
    body: DenAct,
) -> Act<DenAct> {
    let typ = type_key(den_type);
    let parent = Placed {
        path: commission_directory().expect("`commission` is a segment"),
        typ: type_key(DenType::TopLevel),
        level: atta::Level::Public,
    };
    let node = Born {
        key: names::id_segment(commission).expect("an id is a key"),
        typ: typ.clone(),
        name: names::display_name(&name(text)),
        level: picked,
    };
    let shape = Shape::Create {
        parent,
        node,
        entries: Vec::new(),
    };
    Act {
        name: ActName::from("create".to_owned()),
        typ,
        shape,
        body,
    }
}

/// Atta's act of `act_type` built by hand around `body`, making a node of `den_type` keyed `key`
/// and named `text`, Private, under the Private node of `parent_type` at `parent`.
pub(crate) fn hand_built_create_under(
    parent: RealPath,
    parent_type: DenType,
    act_type: DenType,
    den_type: DenType,
    key: Segment,
    text: &str,
    body: DenAct,
) -> Act<DenAct> {
    let parent = Placed {
        path: parent,
        typ: type_key(parent_type),
        level: atta::Level::Private,
    };
    let node = Born {
        key,
        typ: type_key(den_type),
        name: names::display_name(&name(text)),
        level: None,
    };
    let shape = Shape::Create {
        parent,
        node,
        entries: Vec::new(),
    };
    Act {
        name: ActName::from("declare_slots".to_owned()),
        typ: type_key(act_type),
        shape,
        body,
    }
}
