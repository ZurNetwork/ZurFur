//! Call-order recording over the in-memory fakes: wrappers that note each
//! `begin`/`commit`/`rollback` and each external-port call in one shared
//! [`CallLog`], then delegate. A test reads the log to prove what ran before
//! a unit of work opened.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use domain::elements::{
    commission::{FileDownload, FileKey, FileName},
    did::Did,
    handle::Handle,
};
use domain::ports::{
    AccountRepo, ActorIdentityWrites, ChangelogWrites, CharacterWrites, ColumnWrites,
    CommissionRepo, Database, DidMinter, FileStore, UnitOfWork, UserWrites, WorkflowWrites,
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

/// A [`FileStore`] that logs `put`, `get` and `delete` before delegating.
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

    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        self.log.record("delete");
        self.inner.delete(key).await
    }
}
