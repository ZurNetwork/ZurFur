use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use adapter_mem::MemBackend;
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{ChangelogEntry, ChangelogEntryKind},
    did::Did,
    name::NodeNameError,
};
use domain::ports::{
    AccountRepo, ActorIdentityWrites, BugReportWrites, ChangelogWrites, CharacterWrites,
    ColumnWrites, CommissionRepo, Database, NodeRepo, UnitOfWork, UserWrites, WorkflowWrites,
};

use super::*;
use crate::{
    commission::{CommissionError, create},
    common_error::NotFoundEntity,
};

/// The owner every test's commission belongs to.
fn owner() -> UserId {
    UserId::from(Did::from("did:plc:rename-owner".to_owned()))
}

/// A world over the in-memory runtime, with a database the test may swap.
struct World {
    ports: crate::Ports,
    backend: MemBackend,
}

impl World {
    /// The world, its owner recognized as a User.
    async fn new() -> Self {
        let did = Did::from("did:plc:rename-owner".to_owned());
        let built = test_support::runtime::mem(&did).build();
        built
            .backend
            .provision(&did)
            .await
            .expect("the owner is recognized");
        let ports = crate::Ports::from(&built.runtime);
        Self {
            ports,
            backend: built.backend,
        }
    }

    /// Creates a commission titled `title`, owned by [`owner`], and answers its id.
    async fn commission(&self, title: &str) -> CommissionId {
        let command = create::Command {
            actor_id: owner(),
            title: title.parse().expect("a valid title"),
            maturity: None,
            deadline: None,
        };
        let created = Commissions::new(&self.ports)
            .create(command, Utc::now())
            .await
            .expect("creates");
        created.id
    }

    async fn rename(
        &self,
        actor_id: UserId,
        commission_id: CommissionId,
        title: &str,
    ) -> CommissionResult<CommissionValues> {
        let command = Command {
            actor_id,
            commission_id,
            title: title.parse().expect("the title rule keeps it"),
        };
        Commissions::new(&self.ports)
            .rename(command, Utc::now())
            .await
    }

    /// The stored title of `id`.
    async fn stored_title(&self, id: CommissionId) -> String {
        let commission = self
            .ports
            .commissions
            .find(&id)
            .await
            .expect("the store reads")
            .expect("the commission exists");
        commission.title.as_str().to_owned()
    }

    /// Every changelog entry of `id`, in stream order.
    async fn entries(&self, id: CommissionId) -> Vec<ChangelogEntry> {
        self.ports
            .changelog
            .entries(&id)
            .await
            .expect("the store reads")
    }

    /// The kinds of `id`'s entries, in stream order.
    async fn kinds(&self, id: CommissionId) -> Vec<ChangelogEntryKind> {
        let entries = self.entries(id).await;
        entries.iter().map(|entry| entry.kind).collect()
    }

    /// Counts every unit opened from here on, and answers the count.
    fn count_units(&mut self) -> Arc<AtomicUsize> {
        let opened = Arc::new(AtomicUsize::new(0));
        let counting = CountingDatabase {
            inner: self.ports.database.clone(),
            opened: opened.clone(),
        };
        self.ports.database = Arc::new(counting);
        opened
    }
}

/// A [`Database`] that counts the units it opens.
struct CountingDatabase {
    inner: Arc<dyn Database>,
    opened: Arc<AtomicUsize>,
}

#[allow(
    clippy::disallowed_methods,
    reason = "a unit-of-work double delegates to the unit it wraps"
)]
#[async_trait]
impl Database for CountingDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        self.inner.begin().await
    }
}

/// A [`Database`] whose units refuse every changelog append.
struct RefusingChangelogDatabase(Arc<dyn Database>);

#[allow(
    clippy::disallowed_methods,
    reason = "a unit-of-work double delegates to the unit it wraps"
)]
#[async_trait]
impl Database for RefusingChangelogDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let inner = self.0.begin().await?;
        Ok(Box::new(RefusingChangelogUnit(inner)))
    }
}

/// A real unit whose `changelog()` refuses; everything else delegates.
struct RefusingChangelogUnit(Box<dyn UnitOfWork>);

#[allow(
    clippy::disallowed_methods,
    reason = "a unit-of-work double delegates to the unit it wraps"
)]
#[async_trait]
impl UnitOfWork for RefusingChangelogUnit {
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        self.0.accounts()
    }

    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        self.0.commissions()
    }

    // The one interposition.
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        Box::new(RefusingChangelog)
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        self.0.users()
    }

    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        self.0.actor_identities()
    }

    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_> {
        self.0.workflows()
    }

    fn columns(&mut self) -> Box<dyn ColumnWrites + '_> {
        self.0.columns()
    }

    fn characters(&mut self) -> Box<dyn CharacterWrites + '_> {
        self.0.characters()
    }

    fn bug_reports(&mut self) -> Box<dyn BugReportWrites + '_> {
        self.0.bug_reports()
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "a unit-of-work double hands out the node repo of the unit it wraps"
    )]
    fn nodes(&mut self) -> Box<dyn NodeRepo + '_> {
        self.0.nodes()
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.0.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.0.rollback().await
    }
}

/// A changelog that refuses every append.
struct RefusingChangelog;

#[async_trait]
impl ChangelogWrites for RefusingChangelog {
    async fn append(&mut self, _entry: &NewChangelogEntry) -> anyhow::Result<()> {
        anyhow::bail!("the changelog refused the entry")
    }
}

/// A [`Database`] whose units open with the commission already deleted, as if its owner
/// deleted it after the rename's ownership check.
struct VanishingDatabase {
    inner: Arc<dyn Database>,
    commission: CommissionId,
}

#[allow(
    clippy::disallowed_methods,
    reason = "a unit-of-work double delegates to the unit it wraps"
)]
#[async_trait]
impl Database for VanishingDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let mut unit = self.inner.begin().await?;
        unit.commissions().delete(&self.commission).await?;
        Ok(unit)
    }
}

#[tokio::test]
async fn the_owner_renames_the_commission() {
    let world = World::new().await;
    let id = world.commission("A ref sheet").await;

    let renamed = world
        .rename(owner(), id, "A full piece")
        .await
        .expect("renames");

    assert_eq!(renamed.id, id);
    assert_eq!(renamed.title.as_str(), "A full piece");
    assert_eq!(world.stored_title(id).await, "A full piece");
    let expected_kinds = [ChangelogEntryKind::Created, ChangelogEntryKind::Renamed];
    assert_eq!(world.kinds(id).await, expected_kinds);
}

#[tokio::test]
async fn a_participant_who_is_not_the_owner_is_refused() {
    let world = World::new().await;
    let id = world.commission("A ref sheet").await;
    let participant = UserId::from(Did::from("did:plc:rename-participant".to_owned()));
    world
        .backend
        .seed_participant(id, participant.clone(), Utc::now());

    let refused = world.rename(participant, id, "Mine now").await;

    assert!(
        matches!(refused, Err(CommissionError::InsufficientPermissions)),
        "{refused:?}"
    );
    assert_eq!(world.stored_title(id).await, "A ref sheet");
    assert_eq!(world.kinds(id).await, [ChangelogEntryKind::Created]);
}

#[tokio::test]
async fn a_stranger_is_answered_not_a_member() {
    let world = World::new().await;
    let id = world.commission("A ref sheet").await;
    let stranger = UserId::from(Did::from("did:plc:rename-stranger".to_owned()));

    let refused = world.rename(stranger, id, "Mine now").await;

    assert!(
        matches!(refused, Err(CommissionError::NotAMember)),
        "{refused:?}"
    );
    assert_eq!(world.stored_title(id).await, "A ref sheet");
}

#[tokio::test]
async fn a_made_up_id_is_answered_not_found() {
    let world = World::new().await;
    let made_up = CommissionId::from(uuid::Uuid::now_v7());

    let refused = world.rename(owner(), made_up, "Anything").await;

    assert!(
        matches!(
            refused,
            Err(CommissionError::CommonError(CommonError::NotFound(
                NotFoundEntity::Commission
            )))
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_rename_whose_entry_fails_leaves_the_title_unchanged() {
    let mut world = World::new().await;
    let id = world.commission("A ref sheet").await;
    let refusing = RefusingChangelogDatabase(world.ports.database.clone());
    world.ports.database = Arc::new(refusing);

    let failed = world.rename(owner(), id, "A full piece").await;

    assert!(
        matches!(failed, Err(CommissionError::CommonError(_))),
        "{failed:?}"
    );
    assert_eq!(world.stored_title(id).await, "A ref sheet");
    assert_eq!(world.kinds(id).await, [ChangelogEntryKind::Created]);
}

#[tokio::test]
async fn a_commission_deleted_before_the_write_answers_not_found() {
    let mut world = World::new().await;
    let id = world.commission("A ref sheet").await;
    let vanishing = VanishingDatabase {
        inner: world.ports.database.clone(),
        commission: id,
    };
    world.ports.database = Arc::new(vanishing);

    let refused = world.rename(owner(), id, "A full piece").await;

    assert!(
        matches!(
            refused,
            Err(CommissionError::CommonError(CommonError::NotFound(
                NotFoundEntity::Commission
            )))
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn the_rename_entry_is_built_by_its_constructor() {
    let world = World::new().await;
    let id = world.commission("A ref sheet").await;

    world
        .rename(owner(), id, "A full piece")
        .await
        .expect("renames");

    let entries = world.entries(id).await;
    let renamed = entries
        .iter()
        .find(|entry| entry.kind == ChangelogEntryKind::Renamed)
        .expect("a renamed entry");
    let empty_payload = serde_json::json!({});
    assert_eq!(renamed.payload, empty_payload, "the payload names no title");
    assert_eq!(renamed.note, None, "a rename carries no note");
    assert_eq!(renamed.actor_id, Some(owner()));
}

// A title the Den's name rule refuses can't be made, so no rename ever carries one.
#[test]
fn a_title_the_den_name_rule_refuses_is_never_a_title() {
    let slashed = "Ref/sheet".parse::<CommissionTitle>();
    let reversed = "Ref\u{202E}sheet".parse::<CommissionTitle>();

    assert_eq!(slashed, Err(NodeNameError::Slash));
    assert_eq!(reversed, Err(NodeNameError::Invisible));
}

#[tokio::test]
async fn the_answer_is_the_title_as_stored() {
    let world = World::new().await;
    let id = world.commission("A ref sheet").await;

    let renamed = world
        .rename(owner(), id, "Full\u{200B} piece")
        .await
        .expect("renames");

    assert_eq!(renamed.title.as_str(), "Full piece");
    assert_eq!(world.stored_title(id).await, "Full piece");
}

#[tokio::test]
async fn renaming_to_the_same_title_records_nothing() {
    let world = World::new().await;
    let id = world.commission("A ref sheet").await;

    let renamed = world
        .rename(owner(), id, "A ref sheet")
        .await
        .expect("answers the commission");

    assert_eq!(renamed.id, id);
    assert_eq!(renamed.title.as_str(), "A ref sheet");
    assert_eq!(world.kinds(id).await, [ChangelogEntryKind::Created]);
}

#[tokio::test]
async fn a_refused_rename_opens_no_unit() {
    let mut world = World::new().await;
    let id = world.commission("A ref sheet").await;
    let participant = UserId::from(Did::from("did:plc:rename-participant".to_owned()));
    world
        .backend
        .seed_participant(id, participant.clone(), Utc::now());
    let stranger = UserId::from(Did::from("did:plc:rename-stranger".to_owned()));
    let opened = world.count_units();

    let by_stranger = world.rename(stranger, id, "Mine now").await;
    assert!(
        matches!(by_stranger, Err(CommissionError::NotAMember)),
        "{by_stranger:?}"
    );
    assert_eq!(opened.load(Ordering::SeqCst), 0, "a stranger opens no unit");

    let by_participant = world.rename(participant, id, "Mine now").await;
    assert!(
        matches!(
            by_participant,
            Err(CommissionError::InsufficientPermissions)
        ),
        "{by_participant:?}"
    );
    assert_eq!(
        opened.load(Ordering::SeqCst),
        0,
        "a Participant who isn't the owner opens no unit"
    );
}

#[tokio::test]
async fn a_real_rename_opens_one_unit() {
    let mut world = World::new().await;
    let id = world.commission("A ref sheet").await;
    let opened = world.count_units();

    world
        .rename(owner(), id, "A full piece")
        .await
        .expect("renames");

    assert_eq!(opened.load(Ordering::SeqCst), 1);
}
