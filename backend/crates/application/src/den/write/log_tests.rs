//! What the Den's write logs on PostgreSQL, at trace, four ways: the owner's rename, a
//! stranger's, a Participant's (a rename needs the Owner), and a create on a taken segment. No
//! DID or handle at info or above, no name anywhere, and nothing of PostgreSQL's detail.

use std::sync::Arc;

use domain::elements::{
    commission::{CommissionRole, CommissionStanding},
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
    test_acts::{create_act, rename_act},
};

/// The owner's DID.
const OWNER: &str = "did:plc:den-write-log-owner";

/// A stranger's DID.
const STRANGER: &str = "did:plc:den-write-log-stranger";

/// A Participant's DID, their standing scripted on the pool.
const PARTICIPANT: &str = "did:plc:den-write-log-participant";

/// The new name the owner gives, which no line may carry.
const SECRET_NAME: &str = "Quiet secret title";

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
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let renamed = write(&ports, &owner, rename_act(commission, SECRET_NAME)).await;
    let unseen = write(&ports, &stranger, rename_act(commission, SECRET_NAME)).await;
    let forbidden = write(&ports, &participant, rename_act(commission, SECRET_NAME)).await;
    let taken = MINT_AS
        .scope(commission, write(&ports, &owner, create_act(SECRET_NAME)))
        .await;

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
