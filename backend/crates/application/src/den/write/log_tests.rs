//! What the Den's write logs on PostgreSQL, at trace, nine ways: the owner's rename, a
//! stranger's, a Participant's (a rename needs the Owner), a create on a taken segment, the
//! owner's lifecycle move and one its rule refuses, a Delayed flag its rule refuses and a
//! deadline set, and a create whose store failure the gate latches. No DID or handle at info or
//! above, no name anywhere, and nothing of PostgreSQL's detail. The field acts the first test
//! leaves out (the direction set and clear, the Delayed raise and take-down, the deadline clear)
//! log the same, a stranger's of them included; so do the dispute's raise and clear, landed,
//! refused by their rule and a stranger's. The system's Late mark logs no DID and no name at any
//! level.

use std::sync::Arc;

use domain::elements::{
    commission::{
        CommissionRefusal, CommissionRole, CommissionStanding, DirectionStatus, LifecycleStep,
    },
    did::Did,
    level::Level,
};
use test_support::{
    log_capture::CapturedLog,
    pg_app::pg_runtime,
    store_doubles::{ScriptedStanding, StandingScript},
};

use super::tests::{PG_TITLE, node_name, pg_seed, provision, write};
use super::*;
use crate::commission::CommissionError;
use crate::common_error::{CommonError, NotFoundEntity};
use crate::den::{
    authority::MINT_AS,
    test_acts::{
        FieldCall, clear_deadline_act, clear_delayed_act, clear_direction_act, create_act,
        field_call, mark_late_act, move_act, rename_act, set_deadline_act, set_delayed_act,
        set_direction_act, the_system,
    },
};

/// The owner's DID.
const OWNER: &str = "did:plc:den-write-log-owner";

/// A stranger's DID.
const STRANGER: &str = "did:plc:den-write-log-stranger";

/// A Participant's DID, their standing scripted on the pool.
const PARTICIPANT: &str = "did:plc:den-write-log-participant";

/// A DID no user row holds: a create as them breaks the commission's owner key.
const GHOST: &str = "did:plc:den-write-log-ghost";

/// The commission's owner key, which the lent create's insert breaks for [`GHOST`].
const OWNER_KEY: &str = "commission_owner_id_fkey";

/// The new name the owner gives, which no line may carry.
const SECRET_NAME: &str = "Quiet secret title";

/// A log capture at trace, and the guard that makes it this thread's subscriber.
fn capture_at_trace() -> (CapturedLog, tracing::subscriber::DefaultGuard) {
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    (log, guard)
}

#[tokio::test]
async fn the_write_path_logs_no_identity_no_name_and_no_store_detail() {
    let pg = pg_runtime().await;
    let mut ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, OWNER).await;
    let stranger = provision(&*ports.database, STRANGER).await;
    let participant = provision(&*ports.database, PARTICIPANT).await;
    let script = StandingScript::default();
    let participant_standing = CommissionStanding {
        role: Some(CommissionRole::Participant),
        level: Level::Private,
        archived: false,
    };
    script.answer_standing(commission, participant.clone(), Some(participant_standing));
    ports.commissions = Arc::new(ScriptedStanding::new(ports.commissions.clone(), script));
    let (log, _guard) = capture_at_trace();

    let renamed = write(&ports, &owner, rename_act(commission, SECRET_NAME)).await;
    let unseen = write(&ports, &stranger, rename_act(commission, SECRET_NAME)).await;
    let forbidden = write(&ports, &participant, rename_act(commission, SECRET_NAME)).await;
    let taken = MINT_AS
        .scope(commission, write(&ports, &owner, create_act(SECRET_NAME)))
        .await;
    let moved = write(&ports, &owner, move_act(commission, LifecycleStep::Active)).await;
    let refused = write(&ports, &owner, move_act(commission, LifecycleStep::Draft)).await;
    let undated = write(&ports, &owner, set_delayed_act(commission)).await;
    let deadline = "2099-03-01T00:00:00Z".parse().expect("a timestamp");
    let scheduled = write(&ports, &owner, set_deadline_act(commission, deadline)).await;

    assert!(renamed.is_ok(), "{renamed:?}");
    assert!(matches!(unseen, Err(DenWriteError::NotFound)), "{unseen:?}");
    assert!(
        matches!(forbidden, Err(DenWriteError::Forbidden)),
        "{forbidden:?}"
    );
    assert!(
        matches!(
            taken,
            Err(DenWriteError::Refused(RefusalClass::SegmentTaken))
        ),
        "{taken:?}"
    );
    assert!(moved.is_ok(), "{moved:?}");
    assert!(
        matches!(
            refused,
            Err(DenWriteError::Precondition(
                CommissionRefusal::LifecycleMoveRefused
            ))
        ),
        "{refused:?}"
    );
    assert!(
        matches!(
            undated,
            Err(DenWriteError::Precondition(CommissionRefusal::NoDeadline))
        ),
        "{undated:?}"
    );
    assert!(scheduled.is_ok(), "{scheduled:?}");
    assert_eq!(node_name(&*ports.database, commission).await, SECRET_NAME);
    let text = log.text();
    assert!(
        text.contains("use case finished"),
        "the capture saw nothing: {text}"
    );
    for line in text.lines() {
        for leak in [SECRET_NAME, PG_TITLE, "DETAIL", "Key ("] {
            assert!(!line.contains(leak), "a line carries {leak}: {line}");
        }
        let at_info_or_above = [" INFO ", " WARN ", " ERROR "]
            .iter()
            .any(|level| line.contains(level));
        if at_info_or_above {
            for identity in [OWNER, STRANGER, PARTICIPANT, "did:plc:"] {
                assert!(
                    !line.contains(identity),
                    "an info line carries {identity}: {line}"
                );
            }
        }
    }
}

/// The owner of the field acts' commission.
const FIELD_OWNER: &str = "did:plc:den-write-log-field-owner";

/// A stranger to the field acts' commission.
const FIELD_STRANGER: &str = "did:plc:den-write-log-field-stranger";

// The field acts the first test leaves out land for the owner and miss for a stranger; no line
// at any level carries a DID, and no line carries the commission's name.
#[tokio::test]
async fn the_field_acts_log_no_identity_and_no_name() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, FIELD_OWNER).await;
    let stranger = provision(&*ports.database, FIELD_STRANGER).await;
    let deadline = "2099-03-01T00:00:00Z".parse().expect("a timestamp");
    let (log, _guard) = capture_at_trace();

    let dated = write(&ports, &owner, set_deadline_act(commission, deadline)).await;
    let steered = write(
        &ports,
        &owner,
        set_direction_act(commission, DirectionStatus::WaitingForInput),
    )
    .await;
    let delayed = write(&ports, &owner, set_delayed_act(commission)).await;
    let undelayed = write(&ports, &owner, clear_delayed_act(commission)).await;
    let unsteered = write(&ports, &owner, clear_direction_act(commission)).await;
    let undated = write(&ports, &owner, clear_deadline_act(commission)).await;
    let unseen_direction = write(
        &ports,
        &stranger,
        set_direction_act(commission, DirectionStatus::ChangesRequested),
    )
    .await;
    let unseen_delayed = write(&ports, &stranger, set_delayed_act(commission)).await;
    let unseen_clear = write(&ports, &stranger, clear_deadline_act(commission)).await;

    for landed in [&dated, &steered, &delayed, &undelayed, &unsteered, &undated] {
        assert!(landed.is_ok(), "{landed:?}");
    }
    for unseen in [&unseen_direction, &unseen_delayed, &unseen_clear] {
        assert!(matches!(unseen, Err(DenWriteError::NotFound)), "{unseen:?}");
    }
    let text = log.text();
    assert!(
        text.contains("use case finished"),
        "the capture saw nothing: {text}"
    );
    for line in text.lines() {
        for leak in [
            FIELD_OWNER,
            FIELD_STRANGER,
            "did:plc:",
            PG_TITLE,
            "DETAIL",
            "Key (",
        ] {
            assert!(!line.contains(leak), "a line carries {leak}: {line}");
        }
    }
}

/// The owner of the dispute acts' commission.
const DISPUTE_OWNER: &str = "did:plc:den-write-log-dispute-owner";

/// A stranger to the dispute acts' commission.
const DISPUTE_STRANGER: &str = "did:plc:den-write-log-dispute-stranger";

// The dispute's raise and clear, through their typed use cases, land for the owner, are refused
// the second time by their rule, and miss for a stranger; no line at any level carries a DID,
// and no line carries the commission's name.
#[tokio::test]
async fn the_dispute_acts_log_no_identity_and_no_name() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, DISPUTE_OWNER).await;
    let stranger = provision(&*ports.database, DISPUTE_STRANGER).await;
    let now = chrono::Utc::now();
    let (log, _guard) = capture_at_trace();

    let raised = field_call(&ports, &owner, commission, FieldCall::RaiseDispute, now).await;
    let raised_again = field_call(&ports, &owner, commission, FieldCall::RaiseDispute, now).await;
    let unseen_clear =
        field_call(&ports, &stranger, commission, FieldCall::ClearDispute, now).await;
    let cleared = field_call(&ports, &owner, commission, FieldCall::ClearDispute, now).await;
    let cleared_again = field_call(&ports, &owner, commission, FieldCall::ClearDispute, now).await;
    let unseen_raise =
        field_call(&ports, &stranger, commission, FieldCall::RaiseDispute, now).await;

    for landed in [&raised, &cleared] {
        assert!(landed.is_ok(), "{landed:?}");
    }
    assert!(
        matches!(raised_again, Err(CommissionError::CommissionDisputed)),
        "{raised_again:?}"
    );
    assert!(
        matches!(cleared_again, Err(CommissionError::CommissionNotDisputed)),
        "{cleared_again:?}"
    );
    for unseen in [&unseen_raise, &unseen_clear] {
        assert!(
            matches!(
                unseen,
                Err(CommissionError::CommonError(CommonError::NotFound(
                    NotFoundEntity::Commission
                )))
            ),
            "{unseen:?}"
        );
    }
    let text = log.text();
    assert!(
        text.contains("use case finished"),
        "the capture saw nothing: {text}"
    );
    for line in text.lines() {
        for leak in [
            DISPUTE_OWNER,
            DISPUTE_STRANGER,
            "did:plc:",
            PG_TITLE,
            "DETAIL",
            "Key (",
        ] {
            assert!(!line.contains(leak), "a line carries {leak}: {line}");
        }
    }
}

// PostgreSQL refuses the commission's insert on its owner key with a DETAIL naming the DID.
// The latch keeps that failure only as the write error's source, which holds the DETAIL; the
// log, at every level, holds neither it nor the DID.
#[tokio::test]
async fn a_latched_store_failure_logs_no_store_detail_and_no_did() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let ghost = UserId::from(Did::from(GHOST.to_owned()));
    let (log, _guard) = capture_at_trace();

    let written = write(&ports, &ghost, create_act(SECRET_NAME)).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    let kept = format!("{written:#?}");
    for detail in [OWNER_KEY, "Key (", GHOST] {
        assert!(
            kept.contains(detail),
            "the error's source lost {detail}: {kept}"
        );
    }
    let text = log.text();
    assert!(
        text.contains("use case finished"),
        "the capture saw nothing: {text}"
    );
    for line in text.lines() {
        for leak in ["Key (", "DETAIL", GHOST, "did:plc:", SECRET_NAME] {
            assert!(!line.contains(leak), "a line carries {leak}: {line}");
        }
    }
}

/// The system's DID-less write's owner.
const SYSTEM_OWNER: &str = "did:plc:den-write-log-system-owner";

// The system's Late mark, landing and then owing nothing, logs no DID and no name at any level.
#[tokio::test]
async fn the_system_act_logs_no_did_or_name_at_any_level() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, SYSTEM_OWNER).await;
    let missed = chrono::Utc::now() - chrono::Duration::hours(1);
    write(&ports, &owner, set_deadline_act(commission, missed))
        .await
        .expect("the owner sets a deadline that has passed");
    let (log, _guard) = capture_at_trace();

    let marked = system_write(&ports, mark_late_act(commission)).await;
    let owed_nothing = system_write(&ports, mark_late_act(commission)).await;

    assert!(marked.is_ok(), "{marked:?}");
    assert!(
        matches!(
            owed_nothing,
            Err(DenWriteError::Precondition(CommissionRefusal::NothingOwed))
        ),
        "{owed_nothing:?}"
    );
    let text = log.text();
    assert!(
        text.contains("use case finished"),
        "the capture saw nothing: {text}"
    );
    for line in text.lines() {
        for leak in [SYSTEM_OWNER, "did:plc:", PG_TITLE, "DETAIL"] {
            assert!(!line.contains(leak), "a line carries {leak}: {line}");
        }
    }
}

/// The system's write of `act` through the use case.
async fn system_write(ports: &Ports, act: DenAct) -> Result<WriteResult, DenWriteError> {
    let command = Command {
        actor: the_system(),
        act,
    };
    Den::from(ports).write(command, chrono::Utc::now()).await
}
