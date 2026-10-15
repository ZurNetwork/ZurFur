//! What den's check logs, through the use cases that ask it: one debug line per answer with
//! the node's kind and the answer's class, never an id. Alone in its binary: tracing caches a
//! callsite's interest, so a sibling test hitting the check first could blind the capture.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::sync::Arc;

use application::{commission::files::download, transaction, user::me::MeQuery};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, FileKey, Visibility},
    did::Did,
    user::{User, UserId},
};
use domain::ports::{Database, UnitOfWork, UserStore};
use test_support::log_capture::CapturedLog;
use uuid::Uuid;

/// The owner's DID: the check opens their own root.
const OWNER: &str = "did:plc:dencheckowner";

/// A stranger's DID: a listed commission shows them a card.
const STRANGER: &str = "did:plc:dencheckstranger";

/// A commission id nothing is stored under.
const ABSENT: Uuid = Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_0c01);

/// The owner, recognized, and a commission of theirs seeded at `visibility`.
async fn seed(database: &dyn Database, visibility: Visibility) -> (UserId, CommissionId) {
    let owner_did = Did::from(OWNER.to_owned());
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let owner = uow.users().provision(&owner_did).await?;
        let title: CommissionTitle = "Listed piece".parse()?;
        let mut commission = Commission::create(title, owner.id.clone(), Utc::now(), None);
        commission.visibility = visibility;
        let id = commission.id;
        uow.commissions().create(&commission).await?;
        Ok((owner.id, id))
    })
    .await
    .expect("the owner and their commission seed")
}

/// A user store whose every read fails.
struct FailingUsers;

#[async_trait]
impl UserStore for FailingUsers {
    async fn find(&self, _id: &UserId) -> anyhow::Result<Option<User>> {
        Err(anyhow::anyhow!("the user store is down"))
    }

    async fn find_by_did(&self, _did: &Did) -> anyhow::Result<Option<User>> {
        Err(anyhow::anyhow!("the user store is down"))
    }
}

/// A stranger's download from `commission`, to make the check answer for it.
fn stranger_s_download(commission: CommissionId) -> download::Query {
    download::Query {
        actor_id: UserId::from(Did::from(STRANGER.to_owned())),
        commission_id: commission,
        file_id: FileKey::from(Uuid::nil()),
    }
}

#[tokio::test]
async fn each_answer_is_logged_by_its_node_kind_and_class_and_never_an_id() {
    let owner_did = Did::from(OWNER.to_owned());
    let runtime = test_support::runtime::mem(&owner_did).build().runtime;
    let (owner, listed) = seed(&*runtime.database, Visibility::Listed).await;
    let mut failing = runtime.clone();
    failing.users = Arc::new(FailingUsers);
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let own_root = MeQuery {
        user_id: owner.clone(),
    };
    let opened = runtime
        .app()
        .users()
        .me(own_root, &runtime.profile_lookups)
        .await;
    let commissions = runtime.app();
    let carded = commissions
        .commissions()
        .files()
        .download(stranger_s_download(listed))
        .await;
    let absent = CommissionId::from(ABSENT);
    let walked = commissions
        .commissions()
        .files()
        .download(stranger_s_download(absent))
        .await;
    let broken_root = MeQuery {
        user_id: owner.clone(),
    };
    let broken = failing
        .app()
        .users()
        .me(broken_root, &failing.profile_lookups)
        .await;

    assert!(opened.is_ok() && carded.is_err() && walked.is_err() && broken.is_err());
    let text = log.text();
    let answers: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("den check answered"))
        .collect();
    let expected = [
        "node=\"own_root\" answer=\"opens\"",
        "node=\"commission\" answer=\"not_found.card\"",
        "node=\"commission\" answer=\"not_found.walk\"",
        "node=\"own_root\" answer=\"store_failed\"",
    ];
    assert_eq!(answers.len(), expected.len(), "{text}");
    for (line, fields) in answers.iter().zip(expected) {
        assert!(line.contains(fields), "{line} lacks {fields}");
        assert!(line.contains("DEBUG"), "{line}");
        for leak in [
            OWNER,
            STRANGER,
            &listed.to_string(),
            &absent.to_string(),
            "the user store is down",
        ] {
            assert!(
                !line.contains(leak),
                "the answer line carries {leak}: {line}"
            );
        }
    }
}
