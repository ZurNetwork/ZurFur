#![allow(
    clippy::disallowed_methods,
    reason = "a test opens the unit the grant is made in"
)]

use adapter_mem::fixture::{alice, untitled};
use chrono::Utc;
use test_support::recording::{CallLog, RecordingDatabase};

use super::*;
use crate::den::{
    Den, DenType,
    probe::Probe,
    write::{Command, DenWriteError},
};
use crate::test_world;

/// The place of "Untitled", which alice owns.
fn untitled_place() -> Place {
    Place::Commission(CommissionPlace::Commission(untitled()))
}

/// A change anchored at "Untitled" whose act names no role.
fn needing_no_role() -> ActSpec {
    ActSpec {
        word: "unroled",
        den_type: DenType::Commission,
        anchor: Anchor::Thing(untitled_place()),
        needs: None,
        shape: ActShape::Change,
    }
}

/// Whether `answer` is the fail-closed refusal.
fn wrong_grant<T>(answer: &Result<T, DenWriteError>) -> bool {
    matches!(
        answer,
        Err(DenWriteError::Refused(RefusalClass::WrongGrant))
    )
}

// The owner herself is refused: an act that needs no role grants nothing on a thing, on the
// pool and in the unit alike, and through the write no unit opens.
#[tokio::test]
async fn a_thing_anchor_whose_act_needs_no_role_fails_closed() {
    let mut world = test_world::world().await;
    let spec = needing_no_role();

    let admitted = admit(&world.ports, &spec, &alice()).await;
    let mut unit = world.ports.database.begin().await.expect("begin");
    let granted = grant(&mut *unit, &spec, &alice()).await;
    unit.rollback().await.expect("the unit ends");

    assert!(wrong_grant(&admitted), "{admitted:?}");
    assert!(wrong_grant(&granted), "{granted:?}");
    let log = CallLog::default();
    let recording = RecordingDatabase::new(world.ports.database.clone(), log.clone());
    world.ports.database = std::sync::Arc::new(recording);
    let create_on_a_thing = Probe::create("Anchored on a thing")
        .anchored(Anchor::Thing(untitled_place()))
        .act();
    let command = Command {
        actor: alice(),
        act: create_on_a_thing,
    };
    let written = Den::from(&world.ports).write(command, Utc::now()).await;
    assert!(wrong_grant(&written), "{written:?}");
    assert_eq!(log.events(), Vec::<&str>::new());
}
