//! Recording wrappers over the in-memory fakes: each notes its calls in a
//! shared [`CallLog`] (and, for the identity reads, whose data each asked
//! for in a [`SubjectLog`]), then delegates.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::{Account, AccountId, AccountMembership, ListingScope},
    character::Character,
    commission::{FileDownload, FileKey, FileName},
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId},
    profile::Profile,
    role::Role,
    text::StoredText,
    user::{User, UserId},
};
use domain::ports::{
    AccountRepo, AccountStore, ActorIdentityWrites, ChangelogWrites, CharacterStore,
    CharacterWrites, ColumnWrites, CommissionRepo, Database, DidMinter, FileStore, ProfileCache,
    ProfileSource, UnitOfWork, UserStore, UserWrites, WorkflowWrites,
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
