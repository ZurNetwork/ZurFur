//! The Den arms' shared test world: the in-memory fixture as this crate's own ports, and a Den
//! read from any node directly under a top-level directory.

use atta::{Caps, RealPath, Segment};
use domain::elements::{did::Did, user::UserId};

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
    World {
        runtime: built.runtime,
        ports,
    }
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
    let crumbs = resolved.crumbs().iter().map(crumb).count();
    let content = reader.content(&resolved, None).await?;
    let read = Read {
        node,
        crumbs,
        content,
    };
    Ok(read)
}
