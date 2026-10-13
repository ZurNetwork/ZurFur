//! The arms' shared test world: the in-memory fixture as this crate's own ports, a recording
//! commission store, and a Den read from any node directly under a top-level directory.

use std::sync::Arc;

use async_trait::async_trait;
use atta::{Caps, RealPath, Segment};
use domain::{
    elements::{
        commission::{
            Commission, CommissionComposition, CommissionFile, CommissionId, CommissionMarkup,
            CommissionSummary, ElementId, FileKey, FileSummary, GrantLevel, Seat, SeatInvitation,
            Slot,
        },
        did::Did,
        user::UserId,
        workflow::{Column, ColumnId, WorkflowId},
    },
    ports::CommissionStore,
};
use test_support::recording::CallLog;

use crate::{
    Ports,
    den::{
        DenContent, DenContext, DenError, DenNode,
        host::DenHost,
        read::{Reader, crumb},
    },
};

/// The in-memory runtime with the fixture world seeded, and this crate's own ports over it.
pub(crate) struct World {
    pub(crate) runtime: composition::Runtime,
    pub(crate) ports: Ports,
}

/// This crate's own [`Ports`] over a runtime's stores.
pub(crate) fn ports_of(runtime: &composition::Runtime) -> Ports {
    Ports {
        database: runtime.database.clone(),
        users: runtime.users.clone(),
        accounts: runtime.accounts.clone(),
        commissions: runtime.commissions.clone(),
        changelog: runtime.changelog.clone(),
        profile_source: runtime.profile_source.clone(),
        profile_cache: runtime.profile_cache.clone(),
        identity_resolver: runtime.identity_resolver.clone(),
        did_minter: runtime.did_minter.clone(),
        files: runtime.files.clone(),
        workflows: runtime.workflows.clone(),
        columns: runtime.columns.clone(),
        characters: runtime.characters.clone(),
    }
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
    let ports = ports_of(&built.runtime);
    World {
        runtime: built.runtime,
        ports,
    }
}

/// A context for `viewer` over `ports`, as the host hands its arms.
pub(crate) fn context<'a>(ports: &'a Ports, viewer: &'a UserId) -> DenContext<'a> {
    DenContext { ports, viewer }
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

/// A [`CommissionStore`] that logs each read it is asked for before delegating.
pub(crate) struct RecordingCommissions {
    pub(crate) inner: Arc<dyn CommissionStore>,
    pub(crate) log: CallLog,
}

#[async_trait]
impl CommissionStore for RecordingCommissions {
    async fn find(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.log.record("find");
        self.inner.find(id).await
    }

    async fn current_column_of_workflow(
        &self,
        commission: &CommissionId,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<Column>> {
        self.log.record("current_column_of_workflow");
        self.inner
            .current_column_of_workflow(commission, workflow_id)
            .await
    }

    async fn current_position_in_column(
        &self,
        commission: &CommissionId,
        column_id: &ColumnId,
    ) -> anyhow::Result<Option<u8>> {
        self.log.record("current_position_in_column");
        self.inner
            .current_position_in_column(commission, column_id)
            .await
    }

    async fn view_grant(
        &self,
        commission_id: &CommissionId,
        user_id: &UserId,
    ) -> anyhow::Result<Option<GrantLevel>> {
        self.log.record("view_grant");
        self.inner.view_grant(commission_id, user_id).await
    }

    async fn is_participant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        self.log.record("is_participant");
        self.inner.is_participant(commission, user).await
    }

    async fn seats(&self, commission: &CommissionId) -> anyhow::Result<Vec<Seat>> {
        self.log.record("seats");
        self.inner.seats(commission).await
    }

    async fn load_composition(
        &self,
        id: &CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>> {
        self.log.record("load_composition");
        self.inner.load_composition(id).await
    }

    async fn find_file(
        &self,
        commission: &CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>> {
        self.log.record("find_file");
        self.inner.find_file(commission, key).await
    }

    async fn markups_for_file(
        &self,
        commission: &CommissionId,
        file: FileKey,
    ) -> anyhow::Result<Vec<CommissionMarkup>> {
        self.log.record("markups_for_file");
        self.inner.markups_for_file(commission, file).await
    }

    async fn find_pending_seat_invitation(
        &self,
        commission: &CommissionId,
        seat: &ElementId,
        user: &UserId,
    ) -> anyhow::Result<Option<SeatInvitation>> {
        self.log.record("find_pending_seat_invitation");
        self.inner
            .find_pending_seat_invitation(commission, seat, user)
            .await
    }

    async fn list_owned_by(&self, owner: &UserId) -> anyhow::Result<Vec<Commission>> {
        self.log.record("list_owned_by");
        self.inner.list_owned_by(owner).await
    }

    async fn list_participating(&self, user: &UserId) -> anyhow::Result<Vec<CommissionSummary>> {
        self.log.record("list_participating");
        self.inner.list_participating(user).await
    }

    async fn files(&self, commission: &CommissionId) -> anyhow::Result<Vec<FileSummary>> {
        self.log.record("files");
        self.inner.files(commission).await
    }

    async fn slots(&self, commission: &CommissionId) -> anyhow::Result<Vec<Slot>> {
        self.log.record("slots");
        self.inner.slots(commission).await
    }
}
