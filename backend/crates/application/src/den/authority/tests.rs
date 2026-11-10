#![allow(
    clippy::disallowed_methods,
    reason = "a test opens the unit the grant is made in"
)]

use adapter_mem::fixture::{alice, untitled};

use domain::elements::commission::{AsOwner, AsParticipant, AsSystem};
use domain::ports::Creator;

use super::*;
use crate::den::{DenType, gate::issue, test_acts::the_system, write::DenWriteError};
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
fn rename_at(anchor: Anchor, needs: Needs) -> ActSpec {
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
        needs: Needs::Create,
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
    let admitted = admit(&world.ports, spec, &Actor::User(alice())).await;
    let mut unit = world.ports.database.begin().await.expect("begin");
    let granted = grant(&mut *unit, spec, Actor::User(alice())).await;
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
        rename_at(Anchor::Thing(untitled_place()), Needs::Create),
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
        Needs::Role(CommissionRole::Owner),
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
        rename_at(
            Anchor::Thing(user_place),
            Needs::Role(CommissionRole::Owner),
        ),
        create_at(Anchor::TopLevel(TopLevel::User)),
    ];

    for spec in specs {
        let (admitted, granted) = admitted_and_granted(&spec).await;

        assert!(not_found(&admitted), "{spec:?}: {admitted:?}");
        assert!(not_found(&granted), "{spec:?}: {granted:?}");
    }
}

/// The system's Late mark on "Untitled".
fn mark_late() -> ActSpec {
    ActSpec {
        word: "mark_late",
        den_type: DenType::Commission,
        anchor: Anchor::Thing(untitled_place()),
        needs: Needs::System,
        shape: ActShape::Change,
    }
}

/// What the pool's verdict and the grant inside a unit each answer for `spec`, as the system.
async fn admitted_and_granted_to_the_system(
    spec: &ActSpec,
) -> (Result<(), DenWriteError>, Result<Grant, DenWriteError>) {
    let world = test_world::world().await;
    let admitted = admit(&world.ports, spec, &the_system()).await;
    let mut unit = world.ports.database.begin().await.expect("begin");
    let granted = grant(&mut *unit, spec, the_system()).await;
    unit.rollback().await.expect("the unit ends");
    (admitted, granted)
}

// The system's grant leaves no user's role: it issues no Owner's, Participant's or Creator's
// capability, only the system's; and no user's grant issues the system's.
#[tokio::test]
async fn a_system_grant_issues_no_user_role_capability() {
    let (admitted, granted) = admitted_and_granted_to_the_system(&mark_late()).await;
    let system = granted.expect("the system's grant on a bound commission");
    let (_, owner) = admitted_and_granted(&rename_at(
        Anchor::Thing(untitled_place()),
        Needs::Role(CommissionRole::Owner),
    ))
    .await;
    let owner = owner.expect("the owner's grant");

    assert!(admitted.is_ok(), "{admitted:?}");
    assert!(issue::<AsOwner>(&system).is_none());
    assert!(issue::<AsParticipant>(&system).is_none());
    assert!(issue::<Creator>(&system).is_none());
    assert!(issue::<AsSystem>(&system).is_some());
    assert!(issue::<AsSystem>(&owner).is_none());
    assert_eq!(system.user(), None, "the system's grant names no user");
    assert_eq!(system.existing_at_least(CommissionRole::Participant), None);
    assert!(
        matches!(system.kind(), GrantKind::System { commission, .. } if *commission == untitled())
    );
}

// Every user act refuses the system as its actor, on the pool and in the unit alike: a rename,
// a field act and a create.
#[tokio::test]
async fn a_user_act_refuses_the_system_actor() {
    let specs = [
        rename_at(
            Anchor::Thing(untitled_place()),
            Needs::Role(CommissionRole::Owner),
        ),
        ActSpec {
            needs: Needs::Role(CommissionRole::Participant),
            shape: ActShape::Change,
            ..mark_late()
        },
        create_at(Anchor::TopLevel(TopLevel::Commission)),
    ];

    for spec in specs {
        let (admitted, granted) = admitted_and_granted_to_the_system(&spec).await;

        assert!(wrong_grant(&admitted), "{spec:?}: {admitted:?}");
        assert!(wrong_grant(&granted), "{spec:?}: {granted:?}");
    }
}

// The system's act refuses a user, the commission's owner included, on the pool and in the
// unit alike.
#[tokio::test]
async fn the_system_act_refuses_a_user() {
    let (admitted, granted) = admitted_and_granted(&mark_late()).await;

    assert!(wrong_grant(&admitted), "{admitted:?}");
    assert!(wrong_grant(&granted), "{granted:?}");
}

// The system's grant is bound to the node its own locked read answered and carries the facts
// read after the lock; a commission gone since the scan is the one not-found.
#[tokio::test]
async fn the_system_grant_reads_its_facts_under_its_own_lock() {
    let (_, granted) = admitted_and_granted_to_the_system(&mark_late()).await;
    let gone = ActSpec {
        anchor: Anchor::Thing(Place::Commission(CommissionPlace::Commission(
            CommissionId::mint(),
        ))),
        ..mark_late()
    };
    let (_, absent) = admitted_and_granted_to_the_system(&gone).await;

    let granted = granted.expect("the system's grant");
    let facts = granted.late_facts().expect("the system's facts");
    assert!(facts.owed, "nothing recorded yet is owed");
    assert!(granted.bound_node().is_some());
    assert!(not_found(&absent), "{absent:?}");
}
