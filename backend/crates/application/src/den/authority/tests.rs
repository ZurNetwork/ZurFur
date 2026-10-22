#![allow(
    clippy::disallowed_methods,
    reason = "a test opens the unit the grant is made in"
)]

use adapter_mem::fixture::{alice, untitled};

use super::*;
use crate::den::{DenType, write::DenWriteError};
use crate::test_world;

/// The place of "Untitled", which alice owns.
fn untitled_place() -> Place {
    Place::Commission(CommissionPlace::Commission(untitled()))
}

/// A name from test text.
fn name(text: &str) -> domain::elements::name::NodeName {
    domain::elements::name::NodeName::try_from(text).expect("a valid test name")
}

/// A rename anchored at `anchor`, needing `needs`.
fn rename_at(anchor: Anchor, needs: Option<CommissionRole>) -> ActSpec {
    ActSpec {
        word: "rename",
        den_type: DenType::Commission,
        anchor,
        needs,
        shape: ActShape::Rename {
            name: name("A new name"),
        },
    }
}

/// A create anchored at `anchor`.
fn create_at(anchor: Anchor) -> ActSpec {
    ActSpec {
        word: "create",
        den_type: DenType::Commission,
        anchor,
        needs: None,
        shape: ActShape::Create {
            name: name("A new piece"),
            level: None,
            entries: Vec::new(),
        },
    }
}

/// What the pool's verdict and the grant inside a unit each answer for `spec`, as alice.
async fn admitted_and_granted(
    spec: &ActSpec,
) -> (Result<(), DenWriteError>, Result<Grant, DenWriteError>) {
    let world = test_world::world().await;
    let admitted = admit(&world.ports, spec, &alice()).await;
    let mut unit = world.ports.database.begin().await.expect("begin");
    let granted = grant(&mut *unit, spec, &alice()).await;
    unit.rollback().await.expect("the unit ends");
    (admitted, granted)
}

/// Whether `answer` is the fail-closed refusal.
fn wrong_grant<T>(answer: &Result<T, DenWriteError>) -> bool {
    matches!(
        answer,
        Err(DenWriteError::Refused(RefusalClass::WrongGrant))
    )
}

/// Whether `answer` is the one not-found.
fn not_found<T>(answer: &Result<T, DenWriteError>) -> bool {
    matches!(answer, Err(DenWriteError::NotFound))
}

// The owner herself is refused: an act that needs no role grants nothing on a thing, on the
// pool and in the unit alike.
#[tokio::test]
async fn a_thing_anchor_whose_act_needs_no_role_fails_closed() {
    let specs = [
        rename_at(Anchor::Thing(untitled_place()), None),
        create_at(Anchor::Thing(untitled_place())),
    ];

    for spec in specs {
        let (admitted, granted) = admitted_and_granted(&spec).await;

        assert!(wrong_grant(&admitted), "{spec:?}: {admitted:?}");
        assert!(wrong_grant(&granted), "{spec:?}: {granted:?}");
    }
}

// Only a create carries a top-level anchor: a rename anchored at `/commission` is refused by
// rule, on the pool and in the unit alike.
#[tokio::test]
async fn a_rename_spec_with_a_top_level_anchor_is_refused() {
    let spec = rename_at(
        Anchor::TopLevel(TopLevel::Commission),
        Some(CommissionRole::Owner),
    );

    let (admitted, granted) = admitted_and_granted(&spec).await;

    assert!(wrong_grant(&admitted), "{admitted:?}");
    assert!(wrong_grant(&granted), "{granted:?}");
}

// A thing the write has no arm for, and a top-level directory with no create arm, are the one
// not-found on the pool and in the unit alike.
#[tokio::test]
async fn a_place_or_a_top_level_with_no_arm_is_the_one_not_found() {
    let user_place = Place::User(crate::user::den::UserPlace::User(alice()));
    let specs = [
        rename_at(Anchor::Thing(user_place), Some(CommissionRole::Owner)),
        create_at(Anchor::TopLevel(TopLevel::User)),
    ];

    for spec in specs {
        let (admitted, granted) = admitted_and_granted(&spec).await;

        assert!(not_found(&admitted), "{spec:?}: {admitted:?}");
        assert!(not_found(&granted), "{spec:?}: {granted:?}");
    }
}
