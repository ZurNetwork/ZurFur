//! A spy over a real database for the gate's tests: its units record each commission create's
//! birth, each commission read and write by name and each node read by name, can redirect every
//! commission read to another commission or fail it, can answer every lifecycle write as landed,
//! and can refuse every changelog append; everything else delegates.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    commission::{
        AsOwner, AsParticipant, ChannelPointer, Commission, CommissionBirth, CommissionFile,
        CommissionId, CommissionMarkup, DeadlineChange, DelayedChange, DirectionChange, ElementId,
        GrantLevel, LateKinds, LifecycleMove, LockedNode, LockedStanding, NewChangelogEntry,
        NewCommission, NewElement, NewSeat, NewSlot, SeatInvitation, SeatInvitationId, SlotBirth,
        TabId, TabRow,
    },
    maturity::Maturity,
    node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType},
    text::StoredText,
    user::UserId,
};
use domain::ports::{
    AccountRepo, ActorIdentityWrites, BugReportWrites, ChangelogWrites, CharacterWrites,
    ColumnWrites, CommissionReads, CommissionRepo, CommissionWrites, Creator, Database, New,
    NodeReads, NodeRepo, NodeWrites, UnitOfWork, UserWrites, WorkflowWrites, WriteCapability,
};

/// What a spy's units record and change; every clone spies alike.
#[derive(Clone, Default)]
pub(super) struct Spy {
    births: Arc<Mutex<Vec<CommissionBirth>>>,
    /// Each commission read and write the spied repos saw, by name, in order.
    calls: Arc<Mutex<Vec<&'static str>>>,
    /// Each node read the spied units saw, by name, in order.
    node_reads: Arc<Mutex<Vec<&'static str>>>,
    /// Whether every unit's lifecycle write answers landed, whatever the store did.
    lifecycle_lands: bool,
    /// When set, every unit's commission `find` reads this commission instead.
    answer_as: Option<CommissionId>,
    /// Whether every unit's commission `find` fails.
    fail_reads: bool,
    /// Whether every unit's changelog append fails.
    refuse_appends: bool,
}

impl Spy {
    /// A spy whose units read `other` whenever a commission is read.
    pub(super) fn answering_as(other: CommissionId) -> Self {
        Self {
            answer_as: Some(other),
            ..Self::default()
        }
    }

    /// A spy whose units fail every commission read.
    pub(super) fn failing_reads() -> Self {
        Self {
            fail_reads: true,
            ..Self::default()
        }
    }

    /// A spy whose units refuse every changelog append.
    pub(super) fn refusing_appends() -> Self {
        Self {
            refuse_appends: true,
            ..Self::default()
        }
    }

    /// A spy whose units answer every lifecycle write as landed, after the store's own write.
    pub(super) fn landing_every_move() -> Self {
        Self {
            lifecycle_lands: true,
            ..Self::default()
        }
    }

    /// Each commission read and write the spied repos saw so far, by name, in order.
    pub(super) fn recorded_calls(&self) -> Vec<&'static str> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Each node read the spied units saw so far, by name, in order.
    pub(super) fn recorded_node_reads(&self) -> Vec<&'static str> {
        self.node_reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn record_node_read(&self, read: &'static str) {
        self.node_reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(read);
    }

    fn record(&self, call: &'static str) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }

    /// Every create's birth so far, in order.
    pub(super) fn recorded_births(&self) -> Vec<CommissionBirth> {
        self.births().clone()
    }

    fn births(&self) -> MutexGuard<'_, Vec<CommissionBirth>> {
        self.births.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A [`Database`] whose units carry the spy.
pub(super) struct SpyDatabase {
    inner: Arc<dyn Database>,
    spy: Spy,
}

impl SpyDatabase {
    /// Wraps `inner`, its units spied on by `spy`.
    pub(super) fn new(inner: Arc<dyn Database>, spy: Spy) -> Self {
        Self { inner, spy }
    }
}

#[async_trait]
impl Database for SpyDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let inner = self.inner.begin().await?;
        let unit = SpyUnit {
            inner,
            spy: self.spy.clone(),
        };
        Ok(Box::new(unit))
    }
}

/// A real unit whose commission repo and changelog the spy watches.
struct SpyUnit {
    inner: Box<dyn UnitOfWork>,
    spy: Spy,
}

/// A changelog that refuses every append.
struct RefusingChangelog;

#[async_trait]
impl ChangelogWrites for RefusingChangelog {
    async fn append(&mut self, _entry: &NewChangelogEntry) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("the changelog refuses"))
    }
}

#[async_trait]
impl UnitOfWork for SpyUnit {
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        self.inner.accounts()
    }

    // An interposition.
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        let repo = SpyRepo {
            inner: self.inner.commissions(),
            spy: self.spy.clone(),
        };
        Box::new(repo)
    }

    // An interposition.
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        if self.spy.refuse_appends {
            return Box::new(RefusingChangelog);
        }
        self.inner.changelog()
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        self.inner.users()
    }

    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        self.inner.actor_identities()
    }

    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_> {
        self.inner.workflows()
    }

    fn columns(&mut self) -> Box<dyn ColumnWrites + '_> {
        self.inner.columns()
    }

    fn characters(&mut self) -> Box<dyn CharacterWrites + '_> {
        self.inner.characters()
    }

    fn bug_reports(&mut self) -> Box<dyn BugReportWrites + '_> {
        self.inner.bug_reports()
    }

    // An interposition: every node read is recorded.
    fn nodes(&mut self) -> Box<dyn NodeRepo + '_> {
        let nodes = SpyNodes {
            inner: self.inner.nodes(),
            spy: self.spy.clone(),
        };
        Box::new(nodes)
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.rollback().await
    }
}

/// A unit's node repo the spy watches: each read recorded by name; writes delegate.
struct SpyNodes<'a> {
    inner: Box<dyn NodeRepo + 'a>,
    spy: Spy,
}

#[async_trait]
impl NodeReads for SpyNodes<'_> {
    async fn at_top(
        &mut self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        self.spy.record_node_read("at_top");
        self.inner.at_top(node_type, segment).await
    }

    async fn child(
        &mut self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        self.spy.record_node_read("child");
        self.inner.child(parent, segment).await
    }

    async fn children(&mut self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>> {
        self.spy.record_node_read("children");
        self.inner.children(parent).await
    }
}

#[async_trait]
impl NodeWrites for SpyNodes<'_> {
    async fn create(&mut self, node: &NewNode) -> anyhow::Result<()> {
        self.inner.create(node).await
    }

    async fn set_name(&mut self, id: &NodeId, name: &StoredText) -> anyhow::Result<bool> {
        self.inner.set_name(id, name).await
    }

    async fn delete(&mut self, id: &NodeId) -> anyhow::Result<bool> {
        self.inner.delete(id).await
    }
}

/// A unit's commission repo the spy watches; reads and writes otherwise delegate.
struct SpyRepo<'a> {
    inner: Box<dyn CommissionRepo + 'a>,
    spy: Spy,
}

#[async_trait]
impl CommissionReads for SpyRepo<'_> {
    // An interposition: a `find` the spy fails, or redirects to read another commission.
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.spy.record("find");
        if self.spy.fail_reads {
            return Err(anyhow::anyhow!("the commission read fails"));
        }
        let read = self.spy.answer_as.as_ref().unwrap_or(id);
        self.inner.find(read).await
    }

    async fn find_for_update(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.inner.find_for_update(id).await
    }

    async fn standing_for_update(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<LockedStanding>> {
        self.spy.record("standing_for_update");
        self.inner.standing_for_update(commission, user).await
    }

    async fn lock_for_system(
        &mut self,
        commission: &CommissionId,
    ) -> anyhow::Result<Option<LockedNode>> {
        self.spy.record("lock_for_system");
        self.inner.lock_for_system(commission).await
    }

    async fn late_owed(
        &mut self,
        commission: &CommissionId,
        late_kinds: &LateKinds,
    ) -> anyhow::Result<bool> {
        self.spy.record("late_owed");
        self.inner.late_owed(commission, late_kinds).await
    }

    async fn is_participant(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        self.inner.is_participant(commission, user).await
    }

    async fn tab_for_update(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<Option<TabRow>> {
        self.inner.tab_for_update(commission, tab).await
    }
}

#[async_trait]
impl CommissionWrites for SpyRepo<'_> {
    // An interposition: every create's birth is recorded.
    async fn create(
        &mut self,
        capability: &WriteCapability<New<CommissionId>, Creator>,
        birth: &CommissionBirth,
        commission: &NewCommission,
    ) -> anyhow::Result<()> {
        self.spy.record("create");
        self.spy.births().push(birth.clone());
        self.inner.create(capability, birth, commission).await
    }

    // An interposition: every lifecycle write is recorded, and may be answered as landed.
    async fn set_lifecycle(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &LifecycleMove,
    ) -> anyhow::Result<bool> {
        self.spy.record("set_lifecycle");
        let landed = self.inner.set_lifecycle(capability, change).await?;
        Ok(landed || self.spy.lifecycle_lands)
    }

    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()> {
        self.inner.declare_seat(seat).await
    }

    async fn create_seat_invitation(
        &mut self,
        invitation: &SeatInvitation,
    ) -> anyhow::Result<SeatInvitation> {
        self.inner.create_seat_invitation(invitation).await
    }

    async fn revoke_seat_invitation(&mut self, id: &SeatInvitationId) -> anyhow::Result<()> {
        self.inner.revoke_seat_invitation(id).await
    }

    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()> {
        self.inner.add_element(element).await
    }

    async fn remove_element(
        &mut self,
        commission: &CommissionId,
        element: &ElementId,
    ) -> anyhow::Result<()> {
        self.inner.remove_element(commission, element).await
    }

    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()> {
        self.inner.add_file(file).await
    }

    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {
        self.inner.add_markup(markup).await
    }

    // An interposition: every Slot declare is recorded.
    async fn declare_slot(
        &mut self,
        capability: &WriteCapability<CommissionId, AsOwner>,
        birth: &SlotBirth,
        slot: &NewSlot,
    ) -> anyhow::Result<()> {
        self.spy.record("declare_slot");
        self.inner.declare_slot(capability, birth, slot).await
    }

    async fn commission_has_facts(&mut self, id: &CommissionId) -> anyhow::Result<bool> {
        self.inner.commission_has_facts(id).await
    }

    async fn delete(&mut self, id: &CommissionId) -> anyhow::Result<()> {
        self.inner.delete(id).await
    }

    async fn set_archived(
        &mut self,
        id: &CommissionId,
        archived_at: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        self.inner.set_archived(id, archived_at).await
    }

    async fn set_maturity(&mut self, id: &CommissionId, maturity: Maturity) -> anyhow::Result<()> {
        self.inner.set_maturity(id, maturity).await
    }

    async fn set_linked_channel(
        &mut self,
        id: &CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        self.inner.set_linked_channel(id, channel).await
    }

    async fn grant_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        self.inner.grant_view(commission, to_user, level).await
    }

    async fn revoke_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
    ) -> anyhow::Result<bool> {
        self.inner.revoke_view(commission, to_user).await
    }

    async fn set_direction_status(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DirectionChange,
    ) -> anyhow::Result<bool> {
        self.inner.set_direction_status(capability, change).await
    }

    async fn set_deadline(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DeadlineChange,
    ) -> anyhow::Result<bool> {
        self.inner.set_deadline(capability, change).await
    }

    async fn set_deadline_status(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DelayedChange,
    ) -> anyhow::Result<bool> {
        self.inner.set_deadline_status(capability, change).await
    }
}
