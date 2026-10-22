//! Test-only acts: the real create, rename and lifecycle-move acts as the den carries them, a
//! commission's stored state at any step, and Atta's acts built by hand around a real act's
//! body, for the tests that drive the write host below the den's builder.

use atta::{Act, ActName, Born, Placed, RealPath, Shape};
use domain::elements::{
    commission::{Commission, CommissionId, CommissionState, LifecycleStep, NewCommission},
    did::Did,
    name::NodeName,
    user::UserId,
};

use crate::{
    commission::{act::CommissionAct, create, den::CommissionPlace, lifecycle::move_to, rename},
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

/// A stored commission's state at `step`, with no deadline, flag, status or rating.
pub(crate) fn state_at(step: LifecycleStep) -> CommissionState {
    let commission = Commission {
        id: CommissionId::mint(),
        owner_id: UserId::from(Did::from("did:plc:state-owner".to_owned())),
        lifecycle_step: step,
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
