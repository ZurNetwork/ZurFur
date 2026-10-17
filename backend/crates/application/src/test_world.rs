//! The Den arms' shared test world: the in-memory fixture as this crate's own ports, and a Den
//! read from any node directly under a top-level directory; plus the recorded world, ids and
//! seeded targets the tests of commission's reads behind den's check run on.

use std::sync::Arc;

use adapter_mem::MemBackend;
use atta::{Caps, RealPath, Segment};
use domain::{
    elements::{
        commission::{
            Commission, CommissionFile, CommissionId, CommissionTitle, FileKey, FileName,
            Visibility,
        },
        did::Did,
        user::UserId,
    },
    ports::UnitOfWork,
};
use test_support::recording::{
    CallLog, RecordingChangelogStore, RecordingCommissionStore, RecordingFileStore,
};
use uuid::Uuid;

use crate::{
    Ports,
    den::{
        DenContent, DenContext, DenError, DenNode, RootNaming,
        host::DenHost,
        read::{Reader, crumb},
    },
};

/// The in-memory runtime with the fixture world seeded, and this crate's own ports over it.
pub(crate) struct World {
    pub(crate) runtime: composition::Runtime,
    pub(crate) ports: Ports,
}

/// The fixture world: identity's people and Account, then commission's rows.
pub(crate) async fn world() -> World {
    let (world, _) = world_and_backend().await;
    world
}

/// The fixture world, and the in-memory backend under it for seeding what no port writes.
pub(crate) async fn world_and_backend() -> (World, MemBackend) {
    let did = Did::from("did:plc:den-arms-test".to_owned());
    let built = test_support::runtime::mem(&did).build();
    adapter_mem::fixture::seed_identity(&built.backend)
        .await
        .expect("identity's fixture seeds");
    let alice = adapter_mem::fixture::alice();
    let bob = adapter_mem::fixture::bob();
    let supreme_arts = adapter_mem::fixture::supreme_arts();
    adapter_mem::fixture::seed_commission(&built.backend, &alice, &bob, &supreme_arts)
        .await
        .expect("commission's fixture seeds");
    let ports = Ports::from(&built.runtime);
    let world = World {
        runtime: built.runtime,
        ports,
    };
    (world, built.backend)
}

/// A context for `viewer` over `ports`, as the host hands its arms.
pub(crate) fn context<'a>(ports: &'a Ports, viewer: &'a UserId) -> DenContext<'a> {
    DenContext {
        ports,
        viewer,
        root_naming: RootNaming::LookUp,
    }
}

/// A segment from test text.
pub(crate) fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

/// A real path from `/`-separated test text.
pub(crate) fn path(text: &str) -> RealPath {
    text.split('/')
        .filter(|piece| !piece.is_empty())
        .map(segment)
        .collect()
}

/// What a Den read answers for one node.
#[derive(Debug)]
pub(crate) struct Read {
    pub(crate) node: DenNode,
    pub(crate) crumbs: usize,
    pub(crate) content: Option<DenContent>,
}

/// Reads the node at `den_path` below `root`, a node directly under a top-level directory,
/// for `viewer`, as the den Query reads below the viewer's own root.
pub(crate) async fn read(
    ports: &Ports,
    viewer: &UserId,
    include_deleted: bool,
    root: &RealPath,
    den_path: &[&str],
) -> Result<Read, DenError> {
    let host = DenHost::new(ports, viewer, include_deleted);
    let reader = Reader::over(&host, Caps::default());
    let segments: Vec<Segment> = den_path.iter().map(|text| segment(text)).collect();
    let resolved = reader.walk(root, &segments).await?;
    let node = reader.node(resolved.node())?;
    let crumbs = resolved
        .crumbs()
        .iter()
        .map(crumb)
        .collect::<Result<Vec<_>, _>>()?
        .len();
    let content = reader.content(&resolved, None).await?;
    let read = Read {
        node,
        crumbs,
        content,
    };
    Ok(read)
}

/// The fixture world with its commission, file and changelog stores recording every read into
/// the returned log.
pub(crate) async fn recorded() -> (World, CallLog) {
    let World { runtime, mut ports } = world().await;
    let log = CallLog::default();
    let commissions = RecordingCommissionStore::new(ports.commissions.clone(), log.clone());
    let files = RecordingFileStore::new(ports.files.clone(), log.clone());
    let changelog = RecordingChangelogStore::new(ports.changelog.clone(), log.clone());
    ports.commissions = Arc::new(commissions);
    ports.files = Arc::new(files);
    ports.changelog = Arc::new(changelog);
    (World { runtime, ports }, log)
}

/// A user who takes part in nothing in the fixture world.
pub(crate) fn stranger() -> UserId {
    let did = Did::from("did:plc:commission-reads-stranger".to_owned());
    UserId::from(did)
}

/// A commission id nothing is stored under.
pub(crate) fn absent_commission() -> CommissionId {
    CommissionId::from(Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0b01))
}

/// A file key nothing is stored under.
pub(crate) fn absent_file() -> FileKey {
    FileKey::from(Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0b02))
}

/// A commission of alice's that a stranger's pair runs against, and a file in it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Target {
    /// What the target is, for assertion messages.
    pub(crate) label: &'static str,
    /// The commission.
    pub(crate) id: CommissionId,
    /// A file in it.
    pub(crate) file: FileKey,
}

/// The commissions a stranger's pairs run against: "Untitled", Private and live; "Old
/// sketch", Private and archived; and two more archived commissions of alice's, one Public and
/// one Listed.
pub(crate) async fn stranger_targets(world: &World) -> [Target; 4] {
    let [untitled_file, _] = adapter_mem::fixture::untitled_files();
    let untitled = Target {
        label: "Private, live",
        id: adapter_mem::fixture::untitled(),
        file: untitled_file,
    };
    let old_sketch = adapter_mem::fixture::old_sketch();
    let private_archived = Target {
        label: "Private, archived",
        id: old_sketch,
        file: seed_file(world, old_sketch).await,
    };
    let public_archived = Target {
        label: "Public, archived",
        ..seed_alice_s(world, "Public, archived", Visibility::Public, true).await
    };
    let listed_archived = Target {
        label: "Listed, archived",
        ..seed_alice_s(world, "Listed, archived", Visibility::Listed, true).await
    };
    [untitled, private_archived, public_archived, listed_archived]
}

/// Seeds a commission of alice's titled `title` at `visibility`, archived or live, with one
/// file in it; no one else takes part in it.
pub(crate) async fn seed_alice_s(
    world: &World,
    title: &'static str,
    visibility: Visibility,
    archived: bool,
) -> Target {
    let parsed_title = title
        .parse::<CommissionTitle>()
        .expect("a valid test title");
    let now = chrono::Utc::now();
    let mut commission = Commission::create(parsed_title, adapter_mem::fixture::alice(), now, None);
    commission.visibility = visibility;
    let id = commission.id;
    let archived_at = archived.then_some(now);
    world
        .runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().create(&commission).await?;
            if let Some(archived_at) = archived_at {
                uow.commissions()
                    .set_archived(&id, Some(archived_at))
                    .await?;
            }
            Ok(())
        })
        .await
        .expect("the commission seeds");
    let file = seed_file(world, id).await;
    Target {
        label: title,
        id,
        file,
    }
}

/// Seeds a file into `commission`, uploaded by alice, under a fresh key.
pub(crate) async fn seed_file(world: &World, commission: CommissionId) -> FileKey {
    let name = FileName::try_from("seeded.png").expect("a valid file name");
    seed_file_named(world, commission, &name).await
}

/// Seeds a file named `name` into `commission`, uploaded by alice, under a fresh key,
/// straight through the file store, so no use case checks the name.
pub(crate) async fn seed_file_named(
    world: &World,
    commission: CommissionId,
    name: &FileName,
) -> FileKey {
    let key = FileKey::from(Uuid::now_v7());
    let mut bytes: &[u8] = b"bytes of a seeded file";
    world
        .ports
        .files
        .put(key, name, "image/png", &mut bytes)
        .await
        .expect("the file stores");
    let file = CommissionFile {
        id: key,
        commission_id: commission,
        uploaded_by: adapter_mem::fixture::alice(),
        created_at: chrono::Utc::now(),
    };
    world
        .runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| uow.commissions().add_file(&file).await)
        .await
        .expect("the file links to its commission");
    key
}

/// Seeds a Private commission of alice's at the fixed `id`, titled `title`, archived or live,
/// with `others` as Participants beside her; its rows go in straight through the stores.
pub(crate) async fn seed_titled_at(
    world: &World,
    backend: &MemBackend,
    id: Uuid,
    title: &'static str,
    archived: bool,
    others: &[UserId],
) -> CommissionId {
    let parsed_title = title
        .parse::<CommissionTitle>()
        .expect("a valid test title");
    let now = chrono::Utc::now();
    let mut commission = Commission::create(parsed_title, adapter_mem::fixture::alice(), now, None);
    let commission_id = CommissionId::from(id);
    commission.id = commission_id;
    let archived_at = archived.then_some(now);
    world
        .runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().create(&commission).await?;
            if let Some(archived_at) = archived_at {
                uow.commissions()
                    .set_archived(&commission_id, Some(archived_at))
                    .await?;
            }
            Ok(())
        })
        .await
        .expect("the commission seeds");
    for other in others {
        backend.seed_participant(commission_id, other.clone(), now);
    }
    commission_id
}
