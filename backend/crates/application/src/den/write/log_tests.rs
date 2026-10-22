//! What the Den's write logs on PostgreSQL, at trace, nine ways: the owner's rename, a
//! stranger's, a Participant's (a rename needs the Owner), a create on a taken segment, the
//! owner's lifecycle move and one its rule refuses, a Delayed flag its rule refuses and a
//! deadline set, and a create whose store failure the gate latches. No DID or handle at info or
//! above, no name anywhere, and nothing of PostgreSQL's detail.

use std::sync::Arc;

use domain::elements::{
    commission::{CommissionRefusal, CommissionRole, CommissionStanding, LifecycleStep},
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
use crate::den::{
    authority::MINT_AS,
    test_acts::{create_act, move_act, rename_act, set_deadline_act, set_delayed_act},
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
