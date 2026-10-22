//! Recording wrappers over the in-memory fakes: each notes its calls in a
//! shared [`CallLog`] (and, for the identity reads, whose data each asked
//! for in a [`SubjectLog`]), then delegates; plus the lists of reads a refused
//! read must never make.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::{Account, AccountId, AccountMembership, ListingScope},
    character::Character,
    commission::{
        ChangelogEntry, Commission, CommissionComposition, CommissionFile, CommissionId,
        CommissionMarkup, CommissionStanding, CommissionSummary, ElementId, FileDownload, FileKey,
        FileName, FileSummary, GrantLevel, Seat, SeatInvitation, Slot,
    },
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId},
    profile::Profile,
    role::Role,
    text::StoredText,
    user::{User, UserId},
    workflow::{Column, ColumnId, Workflow, WorkflowId, WorkflowSummary},
};
use domain::ports::{
    AccountRepo, AccountStore, ActorIdentityWrites, ChangelogStore, ChangelogWrites,
    CharacterStore, CharacterWrites, ColumnWrites, CommissionRepo, CommissionStore, Database,
    DidMinter, FileStore, ProfileCache, ProfileSource, UnitOfWork, UserStore, UserWrites,
    WorkflowStore, WorkflowWrites,
};
use tokio::io::AsyncRead;

/// An ordered, shared list of event names; clones append to the same list.
#[derive(Clone, Default)]
pub struct CallLog(Arc<Mutex<Vec<&'static str>>>);

impl CallLog {
    /// Appends one event.
    pub fn record(&self, event: &'static str) {
        self.0.lock().expect("call log lock").push(event);
    }

    /// Everything recorded so far, in order.
    pub fn events(&self) -> Vec<&'static str> {
        self.0.lock().expect("call log lock").clone()
    }
}

/// The recorded reads a stranger refused at a commission must never make: the whole row,
/// anything by key, and the changelog's entries.
pub const NEVER_FOR_A_STRANGER: [&str; 6] =
    ["find", "files", "find_file", "filename", "get", "entries"];

/// The recorded reads a file the viewer's check can't find must never make: its row, its name
/// or its bytes, by key.
pub const NEVER_FOR_A_MISSING_FILE: [&str; 3] = ["find_file", "filename", "get"];

/// Asserts `reads` hold none of `never`, naming `label` and the offending read.
pub fn assert_reads_none_of(reads: &[&'static str], never: &[&str], label: &str) {
    for read in never {
        assert!(!reads.contains(read), "{label}: {read} in {reads:?}");
    }
}

/// A [`Database`] that logs `begin`, and hands out a unit that logs `commit`
/// and `rollback`.
pub struct RecordingDatabase {
    inner: Arc<dyn Database>,
    log: CallLog,
}

impl RecordingDatabase {
    /// Wraps `inner`, recording into `log`.
    pub fn new(inner: Arc<dyn Database>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl Database for RecordingDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        self.log.record("begin");
        let inner = self.inner.begin().await?;
        let unit = RecordingUnit {
            inner,
            log: self.log.clone(),
        };
        Ok(Box::new(unit))
    }
}

struct RecordingUnit {
    inner: Box<dyn UnitOfWork>,
    log: CallLog,
}

#[async_trait]
impl UnitOfWork for RecordingUnit {
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        self.inner.accounts()
    }

    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        self.inner.commissions()
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

    fn bug_reports(&mut self) -> Box<dyn domain::ports::BugReportWrites + '_> {
        self.inner.bug_reports()
    }

    fn nodes(&mut self) -> Box<dyn domain::ports::NodeRepo + '_> {
        self.inner.nodes()
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.log.record("commit");
        self.inner.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.log.record("rollback");
        self.inner.rollback().await
    }
}

/// A [`DidMinter`] that logs `mint`, `mint_handleless`, `tombstone` and
/// `update_handle` before delegating.
pub struct RecordingDidMinter {
    inner: Arc<dyn DidMinter>,
    log: CallLog,
}

impl RecordingDidMinter {
    /// Wraps `inner`, recording into `log`.
    pub fn new(inner: Arc<dyn DidMinter>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl DidMinter for RecordingDidMinter {
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did> {
        self.log.record("mint");
        self.inner.mint(handle).await
    }

    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        self.log.record("mint_handleless");
        self.inner.mint_handleless().await
    }

    async fn tombstone(&self, did: &Did) -> anyhow::Result<()> {
        self.log.record("tombstone");
        self.inner.tombstone(did).await
    }

    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()> {
        self.log.record("update_handle");
        self.inner.update_handle(did, handle).await
    }
}

/// A [`FileStore`] that logs `put`, `get`, `filename` and `delete` before
/// delegating.
pub struct RecordingFileStore {
    inner: Arc<dyn FileStore>,
    log: CallLog,
}

impl RecordingFileStore {
    /// Wraps `inner`, recording into `log`.
    pub fn new(inner: Arc<dyn FileStore>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl FileStore for RecordingFileStore {
    async fn put(
        &self,
        key: FileKey,
        filename: &FileName,
        content_type: &str,
        content: &mut (dyn AsyncRead + Send + Unpin),
    ) -> anyhow::Result<u64> {
        self.log.record("put");
        self.inner.put(key, filename, content_type, content).await
    }

    async fn get(&self, key: FileKey) -> anyhow::Result<Option<FileDownload>> {
        self.log.record("get");
        self.inner.get(key).await
    }

    async fn filename(&self, key: FileKey) -> anyhow::Result<Option<StoredText>> {
        self.log.record("filename");
        self.inner.filename(key).await
    }

    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        self.log.record("delete");
        self.inner.delete(key).await
    }
}

/// An ordered, shared list of the actors reads were asked about, as text;
/// clones append to the same list.
#[derive(Clone, Default)]
pub struct SubjectLog(Arc<Mutex<Vec<String>>>);

impl SubjectLog {
    /// Appends one actor.
    pub fn record(&self, subject: impl ToString) {
        self.0
            .lock()
            .expect("subject log lock")
            .push(subject.to_string());
    }

    /// Every actor recorded so far, in order.
    pub fn subjects(&self) -> Vec<String> {
        self.0.lock().expect("subject log lock").clone()
    }
}

/// An [`AccountStore`] that logs each method's name, and the user each read
/// was asked about, before delegating.
pub struct RecordingAccountStore {
    inner: Arc<dyn AccountStore>,
    log: CallLog,
    subjects: SubjectLog,
}

impl RecordingAccountStore {
    /// Wraps `inner`, recording into `log` and `subjects`.
    pub fn new(inner: Arc<dyn AccountStore>, log: CallLog, subjects: SubjectLog) -> Self {
        Self {
            inner,
            log,
            subjects,
        }
    }
}

#[async_trait]
impl AccountStore for RecordingAccountStore {
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        self.log.record("find");
        self.inner.find(id).await
    }

    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>> {
        self.log.record("role_of");
        self.subjects.record(user);
        self.inner.role_of(user, account).await
    }

    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        self.log.record("find_pending_invitation");
        self.subjects.record(invited_user);
        self.inner
            .find_pending_invitation(account, invited_user)
            .await
    }

    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>> {
        self.log.record("find_invitation");
        self.inner.find_invitation(id).await
    }

    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        self.log.record("find_did_by_handle");
        self.inner.find_did_by_handle(handle).await
    }

    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        self.log.record("count_handle_changes_since");
        self.inner.count_handle_changes_since(account, since).await
    }

    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool> {
        self.log.record("handle_reserved_for_other");
        self.inner
            .handle_reserved_for_other(handle, excluding, since)
            .await
    }

    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>> {
        self.log.record("list_for_user");
        self.subjects.record(user);
        self.inner.list_for_user(user, scope).await
    }

    async fn list_deactivated_for_owner(&self, owner: &UserId) -> anyhow::Result<Vec<Account>> {
        self.log.record("list_deactivated_for_owner");
        self.subjects.record(owner);
        self.inner.list_deactivated_for_owner(owner).await
    }
}

/// A [`UserStore`] that logs each method's name, and the user or DID it was
/// asked about, before delegating.
pub struct RecordingUserStore {
    inner: Arc<dyn UserStore>,
    log: CallLog,
    subjects: SubjectLog,
}

impl RecordingUserStore {
    /// Wraps `inner`, recording into `log` and `subjects`.
    pub fn new(inner: Arc<dyn UserStore>, log: CallLog, subjects: SubjectLog) -> Self {
        Self {
            inner,
            log,
            subjects,
        }
    }
}

#[async_trait]
impl UserStore for RecordingUserStore {
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        self.log.record("find_user");
        self.subjects.record(id);
        self.inner.find(id).await
    }

    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        self.log.record("find_user_by_did");
        self.subjects.record(did);
        self.inner.find_by_did(did).await
    }
}

/// A [`CharacterStore`] that logs `list_kept_by`, and the keeper it was asked
/// about, before delegating.
pub struct RecordingCharacterStore {
    inner: Arc<dyn CharacterStore>,
    log: CallLog,
    subjects: SubjectLog,
}

impl RecordingCharacterStore {
    /// Wraps `inner`, recording into `log` and `subjects`.
    pub fn new(inner: Arc<dyn CharacterStore>, log: CallLog, subjects: SubjectLog) -> Self {
        Self {
            inner,
            log,
            subjects,
        }
    }
}

#[async_trait]
impl CharacterStore for RecordingCharacterStore {
    async fn list_kept_by(&self, keeper: &UserId) -> anyhow::Result<Vec<Character>> {
        self.log.record("list_kept_by");
        self.subjects.record(keeper);
        self.inner.list_kept_by(keeper).await
    }
}

/// A [`ProfileCache`] that logs `get_profile` and `put_profile`, and the DID
/// each was about, before delegating.
pub struct RecordingProfileCache {
    inner: Arc<dyn ProfileCache>,
    log: CallLog,
    subjects: SubjectLog,
}

impl RecordingProfileCache {
    /// Wraps `inner`, recording into `log` and `subjects`.
    pub fn new(inner: Arc<dyn ProfileCache>, log: CallLog, subjects: SubjectLog) -> Self {
        Self {
            inner,
            log,
            subjects,
        }
    }
}

#[async_trait]
impl ProfileCache for RecordingProfileCache {
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>> {
        self.log.record("get_profile");
        self.subjects.record(did);
        self.inner.get(did).await
    }

    async fn put(&self, profile: &Profile) -> anyhow::Result<()> {
        self.log.record("put_profile");
        self.subjects.record(&profile.did);
        self.inner.put(profile).await
    }
}

/// A [`ProfileSource`] that logs `fetch_profile`, and the DID it was asked
/// about, before delegating.
pub struct RecordingProfileSource {
    inner: Arc<dyn ProfileSource>,
    log: CallLog,
    subjects: SubjectLog,
}

impl RecordingProfileSource {
    /// Wraps `inner`, recording into `log` and `subjects`.
    pub fn new(inner: Arc<dyn ProfileSource>, log: CallLog, subjects: SubjectLog) -> Self {
        Self {
            inner,
            log,
            subjects,
        }
    }
}

#[async_trait]
impl ProfileSource for RecordingProfileSource {
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        self.log.record("fetch_profile");
        self.subjects.record(did);
        self.inner.fetch(did).await
    }
}

/// A [`CommissionStore`] that logs each read by its method name before
/// delegating.
pub struct RecordingCommissionStore {
    inner: Arc<dyn CommissionStore>,
    log: CallLog,
}

impl RecordingCommissionStore {
    /// Wraps `inner`, recording into `log`.
    pub fn new(inner: Arc<dyn CommissionStore>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl CommissionStore for RecordingCommissionStore {
    async fn find(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.log.record("find");
        self.inner.find(id).await
    }

    async fn find_summary(&self, id: &CommissionId) -> anyhow::Result<Option<CommissionSummary>> {
        self.log.record("find_summary");
        self.inner.find_summary(id).await
    }

    async fn standing(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<CommissionStanding>> {
        self.log.record("standing");
        self.inner.standing(commission, user).await
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

/// A [`ChangelogStore`] that logs `entries` before delegating.
pub struct RecordingChangelogStore {
    inner: Arc<dyn ChangelogStore>,
    log: CallLog,
}

impl RecordingChangelogStore {
    /// Wraps `inner`, recording into `log`.
    pub fn new(inner: Arc<dyn ChangelogStore>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl ChangelogStore for RecordingChangelogStore {
    async fn entries(&self, commission: &CommissionId) -> anyhow::Result<Vec<ChangelogEntry>> {
        self.log.record("entries");
        self.inner.entries(commission).await
    }
}

/// A [`WorkflowStore`] that logs each read before delegating: `find` as `find_workflow`, so
/// it reads apart from the commission store's `find` in a shared log, the others by name.
pub struct RecordingWorkflowStore {
    inner: Arc<dyn WorkflowStore>,
    log: CallLog,
}

impl RecordingWorkflowStore {
    /// Wraps `inner`, recording into `log`.
    pub fn new(inner: Arc<dyn WorkflowStore>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl WorkflowStore for RecordingWorkflowStore {
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
        self.log.record("find_workflow");
        self.inner.find(workflow_id).await
    }

    async fn owning_account_of(
        &self,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<AccountId>> {
        self.log.record("owning_account_of");
        self.inner.owning_account_of(workflow_id).await
    }

    async fn columns(&self, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>> {
        self.log.record("columns");
        self.inner.columns(workflow_id).await
    }

    async fn list_for_account(&self, account: &AccountId) -> anyhow::Result<Vec<WorkflowSummary>> {
        self.log.record("list_for_account");
        self.inner.list_for_account(account).await
    }
}

#[cfg(test)]
mod tests;
