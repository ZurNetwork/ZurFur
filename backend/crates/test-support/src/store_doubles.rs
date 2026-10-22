//! Store doubles that answer from a real store until a test scripts them otherwise: an Account
//! listing that also lists a membership the store wouldn't, an Account or user read that fails,
//! a role read that answers a stale role, a user read that finds no row, and a commission
//! standing scripted apart on the pool and in a unit. A script is shared, so a test seeds
//! through the real answers first and scripts the double afterwards.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::{Account, AccountId, AccountMembership, ListingScope},
    commission::{
        ChannelPointer, Commission, CommissionBirth, CommissionComposition, CommissionFile,
        CommissionId, CommissionMarkup, CommissionStanding, CommissionSummary, CommissionTitle,
        DeadlineStatus, DirectionStatus, ElementId, FileKey, FileSummary, GrantLevel,
        LapsedDeadline, NewCommission, NewElement, NewSeat, NewSlot, Seat, SeatInvitation,
        SeatInvitationId, Slot, TabId, TabRow,
    },
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId},
    maturity::Maturity,
    role::Role,
    user::{User, UserId},
    workflow::{Column, ColumnId, WorkflowId},
};
use domain::ports::{
    AccountRepo, AccountStore, ActorIdentityWrites, BugReportWrites, ChangelogWrites,
    CharacterWrites, ColumnWrites, CommissionReads, CommissionRepo, CommissionStore,
    CommissionWrites, Creator, Database, New, NodeRepo, UnitOfWork, UserStore, UserWrites,
    WorkflowWrites, WriteCapability,
};

/// An [`AccountStore`] read a script can make fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountRead {
    /// `find`, one live Account by id.
    Find,
    /// `list_for_user`, a user's memberships.
    ListForUser,
}

/// Makes the membership a scripted listing adds; a membership is not `Clone`.
type MakeMembership = Arc<dyn Fn() -> AccountMembership + Send + Sync>;

/// What a [`ScriptedAccounts`] does beyond its inner store.
#[derive(Default)]
struct AccountsPlan {
    also_listed: Option<MakeMembership>,
    find_fails: bool,
    list_for_user_fails: bool,
    answered_roles: HashMap<(UserId, AccountId), Role>,
}

impl AccountsPlan {
    /// Whether `read` is scripted to fail.
    fn fails(&mut self, read: AccountRead) -> &mut bool {
        match read {
            AccountRead::Find => &mut self.find_fails,
            AccountRead::ListForUser => &mut self.list_for_user_fails,
        }
    }
}

/// A shared script for [`ScriptedAccounts`]; every clone scripts the same double. Empty, the
/// double answers as its inner store does.
#[derive(Clone, Default)]
pub struct AccountsScript(Arc<Mutex<AccountsPlan>>);

impl AccountsScript {
    /// From now on `list_for_user` also lists the membership `make` makes, between the store's
    /// first and second rows, whoever asks.
    pub fn also_list(&self, make: impl Fn() -> AccountMembership + Send + Sync + 'static) {
        self.plan().also_listed = Some(Arc::new(make));
    }

    /// From now on `read` fails, until [`AccountsScript::read_through`].
    pub fn fail(&self, read: AccountRead) {
        *self.plan().fails(read) = true;
    }

    /// From now on `read` answers from the inner store again.
    pub fn read_through(&self, read: AccountRead) {
        *self.plan().fails(read) = false;
    }

    /// From now on `role_of` answers `role` for `user` in `account`, as a read taken before a
    /// concurrent change would; every other pair reads through.
    pub fn answer_role_of(&self, user: UserId, account: AccountId, role: Role) {
        self.plan().answered_roles.insert((user, account), role);
    }

    fn plan(&self) -> MutexGuard<'_, AccountsPlan> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The role scripted for `user` in `account`, else `None` to read through.
    fn answered_role(&self, user: &UserId, account: &AccountId) -> Option<Role> {
        let pair = (user.clone(), account.clone());
        self.plan().answered_roles.get(&pair).cloned()
    }

    fn check(&self, read: AccountRead) -> anyhow::Result<()> {
        if *self.plan().fails(read) {
            let failure = anyhow::anyhow!("the account store failed ({read:?}, scripted)");
            return Err(failure);
        }
        Ok(())
    }

    fn also_listed(&self) -> Option<AccountMembership> {
        let make = self.plan().also_listed.clone()?;
        Some(make())
    }
}

/// An [`AccountStore`] that answers from `inner` as its [`AccountsScript`] says.
pub struct ScriptedAccounts {
    inner: Arc<dyn AccountStore>,
    script: AccountsScript,
}

impl ScriptedAccounts {
    /// Wraps `inner`, answering as `script` says.
    pub fn new(inner: Arc<dyn AccountStore>, script: AccountsScript) -> Self {
        Self { inner, script }
    }
}

#[async_trait]
impl AccountStore for ScriptedAccounts {
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        self.script.check(AccountRead::Find)?;
        self.inner.find(id).await
    }

    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>> {
        if let Some(role) = self.script.answered_role(user, account) {
            return Ok(Some(role));
        }
        self.inner.role_of(user, account).await
    }

    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        self.inner
            .find_pending_invitation(account, invited_user)
            .await
    }

    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>> {
        self.inner.find_invitation(id).await
    }

    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        self.inner.find_did_by_handle(handle).await
    }

    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        self.inner.count_handle_changes_since(account, since).await
    }

    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool> {
        self.inner
            .handle_reserved_for_other(handle, excluding, since)
            .await
    }

    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>> {
        self.script.check(AccountRead::ListForUser)?;
        let mut memberships = self.inner.list_for_user(user, scope).await?;
        if let Some(membership) = self.script.also_listed() {
            let between_first_and_second = memberships.len().min(1);
            memberships.insert(between_first_and_second, membership);
        }
        Ok(memberships)
    }

    async fn list_deactivated_for_owner(&self, owner: &UserId) -> anyhow::Result<Vec<Account>> {
        self.inner.list_deactivated_for_owner(owner).await
    }
}

/// A [`UserStore`] read a script can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRead {
    /// `find`, a user by id.
    Find,
    /// `find_by_did`, a user by DID.
    FindByDid,
}

/// How a scripted user read answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAnswer {
    /// No row, whoever is asked about.
    Gone,
    /// The store fails.
    Fails,
}

/// What a [`ScriptedUsers`] does beyond its inner store.
#[derive(Debug, Default)]
struct UsersPlan {
    find: Option<UserAnswer>,
    find_by_did: Option<UserAnswer>,
}

impl UsersPlan {
    /// How `read` is scripted to answer; `None` reads through.
    fn answer(&mut self, read: UserRead) -> &mut Option<UserAnswer> {
        match read {
            UserRead::Find => &mut self.find,
            UserRead::FindByDid => &mut self.find_by_did,
        }
    }
}

/// A shared script for [`ScriptedUsers`]; every clone scripts the same double. Empty, the double
/// answers as its inner store does.
#[derive(Debug, Clone, Default)]
pub struct UsersScript(Arc<Mutex<UsersPlan>>);

impl UsersScript {
    /// From now on `read` answers as `answer` says, until [`UsersScript::read_through`].
    pub fn answer(&self, read: UserRead, answer: UserAnswer) {
        *self.plan().answer(read) = Some(answer);
    }

    /// From now on `read` answers from the inner store again.
    pub fn read_through(&self, read: UserRead) {
        *self.plan().answer(read) = None;
    }

    fn plan(&self) -> MutexGuard<'_, UsersPlan> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn scripted(&self, read: UserRead) -> Option<UserAnswer> {
        *self.plan().answer(read)
    }
}

/// A [`UserStore`] that answers from `inner` as its [`UsersScript`] says.
pub struct ScriptedUsers {
    inner: Arc<dyn UserStore>,
    script: UsersScript,
}

impl ScriptedUsers {
    /// Wraps `inner`, answering as `script` says.
    pub fn new(inner: Arc<dyn UserStore>, script: UsersScript) -> Self {
        Self { inner, script }
    }

    /// The scripted answer for `read`, else `None` to read through.
    fn scripted(&self, read: UserRead) -> Option<anyhow::Result<Option<User>>> {
        let answer = self.script.scripted(read)?;
        let scripted = match answer {
            UserAnswer::Gone => Ok(None),
            UserAnswer::Fails => Err(anyhow::anyhow!(
                "the user store failed ({read:?}, scripted)"
            )),
        };
        Some(scripted)
    }
}

#[async_trait]
impl UserStore for ScriptedUsers {
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        if let Some(scripted) = self.scripted(UserRead::Find) {
            return scripted;
        }
        self.inner.find(id).await
    }

    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        if let Some(scripted) = self.scripted(UserRead::FindByDid) {
            return scripted;
        }
        self.inner.find_by_did(did).await
    }
}

/// Scripted standing answers, keyed by commission and viewer.
type StandingAnswers = HashMap<(CommissionId, UserId), Option<CommissionStanding>>;

/// What a scripted standing double answers beyond its inner store: the pool's and the unit's
/// answers apart, so the two can disagree.
#[derive(Default)]
struct StandingPlan {
    standing: StandingAnswers,
    standing_for_update: StandingAnswers,
}

/// A shared script for [`ScriptedStanding`] and [`ScriptedStandingDatabase`]; every clone
/// scripts the same doubles. Empty, both answer as their inner store does.
#[derive(Clone, Default)]
pub struct StandingScript(Arc<Mutex<StandingPlan>>);

impl StandingScript {
    /// From now on the pool's `standing` answers `answer` for `user` on `commission`; every
    /// other pair reads through.
    pub fn answer_standing(
        &self,
        commission: CommissionId,
        user: UserId,
        answer: Option<CommissionStanding>,
    ) {
        self.plan().standing.insert((commission, user), answer);
    }

    /// From now on a unit's `standing_for_update` answers `answer` for `user` on `commission`,
    /// without reading through; every other pair reads through.
    pub fn answer_standing_for_update(
        &self,
        commission: CommissionId,
        user: UserId,
        answer: Option<CommissionStanding>,
    ) {
        self.plan()
            .standing_for_update
            .insert((commission, user), answer);
    }

    fn plan(&self) -> MutexGuard<'_, StandingPlan> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The pool answer scripted for the pair, else `None` to read through.
    fn standing(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> Option<Option<CommissionStanding>> {
        let pair = (*commission, user.clone());
        self.plan().standing.get(&pair).copied()
    }

    /// The unit answer scripted for the pair, else `None` to read through.
    fn standing_for_update(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> Option<Option<CommissionStanding>> {
        let pair = (*commission, user.clone());
        self.plan().standing_for_update.get(&pair).copied()
    }
}

/// A [`CommissionStore`] whose `standing` answers as its [`StandingScript`] says; every other
/// read delegates to `inner`.
pub struct ScriptedStanding {
    inner: Arc<dyn CommissionStore>,
    script: StandingScript,
}

impl ScriptedStanding {
    /// Wraps `inner`, answering `standing` as `script` says.
    pub fn new(inner: Arc<dyn CommissionStore>, script: StandingScript) -> Self {
        Self { inner, script }
    }
}

#[async_trait]
impl CommissionStore for ScriptedStanding {
    async fn find(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.inner.find(id).await
    }

    async fn find_summary(&self, id: &CommissionId) -> anyhow::Result<Option<CommissionSummary>> {
        self.inner.find_summary(id).await
    }

    // The one interposition.
    async fn standing(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<CommissionStanding>> {
        if let Some(scripted) = self.script.standing(commission, user) {
            return Ok(scripted);
        }
        self.inner.standing(commission, user).await
    }

    async fn current_column_of_workflow(
        &self,
        commission: &CommissionId,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<Column>> {
        self.inner
            .current_column_of_workflow(commission, workflow_id)
            .await
    }

    async fn current_position_in_column(
        &self,
        commission: &CommissionId,
        column_id: &ColumnId,
    ) -> anyhow::Result<Option<u8>> {
        self.inner
            .current_position_in_column(commission, column_id)
            .await
    }

    async fn view_grant(
        &self,
        commission_id: &CommissionId,
        user_id: &UserId,
    ) -> anyhow::Result<Option<GrantLevel>> {
        self.inner.view_grant(commission_id, user_id).await
    }

    async fn is_participant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        self.inner.is_participant(commission, user).await
    }

    async fn seats(&self, commission: &CommissionId) -> anyhow::Result<Vec<Seat>> {
        self.inner.seats(commission).await
    }

    async fn load_composition(
        &self,
        id: &CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>> {
        self.inner.load_composition(id).await
    }

    async fn find_file(
        &self,
        commission: &CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>> {
        self.inner.find_file(commission, key).await
    }

    async fn markups_for_file(
        &self,
        commission: &CommissionId,
        file: FileKey,
    ) -> anyhow::Result<Vec<CommissionMarkup>> {
        self.inner.markups_for_file(commission, file).await
    }

    async fn find_pending_seat_invitation(
        &self,
        commission: &CommissionId,
        seat: &ElementId,
        user: &UserId,
    ) -> anyhow::Result<Option<SeatInvitation>> {
        self.inner
            .find_pending_seat_invitation(commission, seat, user)
            .await
    }

    async fn list_owned_by(&self, owner: &UserId) -> anyhow::Result<Vec<Commission>> {
        self.inner.list_owned_by(owner).await
    }

    async fn list_participating(&self, user: &UserId) -> anyhow::Result<Vec<CommissionSummary>> {
        self.inner.list_participating(user).await
    }

    async fn files(&self, commission: &CommissionId) -> anyhow::Result<Vec<FileSummary>> {
        self.inner.files(commission).await
    }

    async fn slots(&self, commission: &CommissionId) -> anyhow::Result<Vec<Slot>> {
        self.inner.slots(commission).await
    }
}

/// A [`Database`] whose units answer `standing_for_update` as its [`StandingScript`] says;
/// everything else on the unit delegates to the unit `inner` opens.
pub struct ScriptedStandingDatabase {
    inner: Arc<dyn Database>,
    script: StandingScript,
}

impl ScriptedStandingDatabase {
    /// Wraps `inner`, its units answering `standing_for_update` as `script` says.
    pub fn new(inner: Arc<dyn Database>, script: StandingScript) -> Self {
        Self { inner, script }
    }
}

#[async_trait]
impl Database for ScriptedStandingDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let inner = self.inner.begin().await?;
        let unit = ScriptedStandingUnit {
            inner,
            script: self.script.clone(),
        };
        Ok(Box::new(unit))
    }
}

/// A real unit whose commission repo answers `standing_for_update` as scripted.
struct ScriptedStandingUnit {
    inner: Box<dyn UnitOfWork>,
    script: StandingScript,
}

#[async_trait]
impl UnitOfWork for ScriptedStandingUnit {
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        self.inner.accounts()
    }

    // The one interposition.
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        let repo = ScriptedStandingRepo {
            inner: self.inner.commissions(),
            script: self.script.clone(),
        };
        Box::new(repo)
    }

    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
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

/// A unit's commission repo whose `standing_for_update` answers as scripted; reads and writes
/// otherwise delegate to the real repo.
struct ScriptedStandingRepo<'a> {
    inner: Box<dyn CommissionRepo + 'a>,
    script: StandingScript,
}

#[async_trait]
impl CommissionReads for ScriptedStandingRepo<'_> {
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.inner.find(id).await
    }

    async fn find_for_update(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.inner.find_for_update(id).await
    }

    // The one interposition.
    async fn standing_for_update(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<CommissionStanding>> {
        if let Some(scripted) = self.script.standing_for_update(commission, user) {
            return Ok(scripted);
        }
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
impl CommissionWrites for ScriptedStandingRepo<'_> {
    async fn create(
        &mut self,
        capability: &WriteCapability<New<CommissionId>, Creator>,
        birth: &CommissionBirth,
        commission: &NewCommission,
    ) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests;
