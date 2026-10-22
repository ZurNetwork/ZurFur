//! A spy over a real database for the gate's tests: its units record each commission create's
//! birth, can redirect every commission read to another commission, and can refuse every
//! changelog append; everything else delegates.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    commission::{
        ChannelPointer, Commission, CommissionBirth, CommissionFile, CommissionId,
        CommissionMarkup, CommissionStanding, CommissionTitle, DeadlineStatus, DirectionStatus,
        ElementId, GrantLevel, LapsedDeadline, NewChangelogEntry, NewCommission, NewElement,
        NewSeat, NewSlot, SeatInvitation, SeatInvitationId, TabId, TabRow,
    },
    maturity::Maturity,
    user::UserId,
};
use domain::ports::{
    AccountRepo, ActorIdentityWrites, BugReportWrites, ChangelogWrites, CharacterWrites,
    ColumnWrites, CommissionReads, CommissionRepo, CommissionWrites, Creator, Database, New,
    NodeRepo, UnitOfWork, UserWrites, WorkflowWrites, WriteCapability,
};

/// What a spy's units record and change; every clone spies alike.
#[derive(Clone, Default)]
pub(super) struct Spy {
    births: Arc<Mutex<Vec<CommissionBirth>>>,
    /// When set, every unit's commission `find` reads this commission instead.
    answer_as: Option<CommissionId>,
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

    /// A spy whose units refuse every changelog append.
    pub(super) fn refusing_appends() -> Self {
        Self {
            refuse_appends: true,
            ..Self::default()
        }
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

    fn nodes(&mut self) -> Box<dyn NodeRepo + '_> {
        self.inner.nodes()
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.rollback().await
    }
}

/// A unit's commission repo the spy watches; reads and writes otherwise delegate.
struct SpyRepo<'a> {
    inner: Box<dyn CommissionRepo + 'a>,
    spy: Spy,
}

#[async_trait]
impl CommissionReads for SpyRepo<'_> {
    // An interposition: a `find` the spy redirects reads another commission.
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
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
    ) -> anyhow::Result<Option<CommissionStanding>> {
        self.inner.standing_for_update(commission, user).await
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
        self.spy.births().push(birth.clone());
        self.inner.create(capability, birth, commission).await
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

    async fn declare_slots(&mut self, slots: &[NewSlot]) -> anyhow::Result<()> {
        self.inner.declare_slots(slots).await
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

    async fn set_title(
        &mut self,
        id: &CommissionId,
        title: &CommissionTitle,
    ) -> anyhow::Result<bool> {
        self.inner.set_title(id, title).await
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
        id: &CommissionId,
        status: Option<DirectionStatus>,
    ) -> anyhow::Result<bool> {
        self.inner.set_direction_status(id, status).await
    }

    async fn set_deadline(
        &mut self,
        id: &CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        self.inner.set_deadline(id, deadline).await
    }

    async fn set_deadline_status(
        &mut self,
        id: &CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<bool> {
        self.inner.set_deadline_status(id, status).await
    }

    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>> {
        self.inner.lapsed_deadlines(now).await
    }
}
