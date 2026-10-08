//! In-memory fake of the [`IdentityResolver`] port: a staged world of handles
//! and DID documents, resolved by the port's both-ways rules with no network.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use domain::elements::did::Did;
use domain::elements::handle::AtHandle;
use domain::ports::{IdentityResolver, ResolveError};

/// What a lookup of one handle or one DID document answers.
#[derive(Debug, Clone)]
enum Answer<T> {
    /// The lookup succeeded with this value.
    Found(T),
    /// The host broke a fetch rule.
    Refused,
    /// The lookup could not finish.
    Unavailable,
}

impl<T> Answer<T> {
    /// The found value, or the matching [`ResolveError`].
    fn into_result(self) -> Result<T, ResolveError> {
        match self {
            Self::Found(value) => Ok(value),
            Self::Refused => Err(refused()),
            Self::Unavailable => Err(unavailable()),
        }
    }
}

fn refused() -> ResolveError {
    ResolveError::Refused(anyhow::anyhow!("fetch rule broken (fake)"))
}

fn unavailable() -> ResolveError {
    ResolveError::Unavailable(anyhow::anyhow!("lookup did not finish (fake)"))
}

/// In-process [`IdentityResolver`] over a staged world; cloning shares it.
/// A handle names a DID, a DID's document claims at most one handle, and both
/// must agree for a lookup to confirm. Anything unstaged is not found.
#[derive(Clone, Default)]
pub struct MemIdentityResolver {
    /// `handle → the DID it names` (the DNS record or well-known file).
    handles: Arc<Mutex<HashMap<AtHandle, Answer<Did>>>>,
    /// `DID → the handle its document claims, if any`.
    documents: Arc<Mutex<HashMap<Did, Answer<Option<AtHandle>>>>>,
    /// Set by [`MemIdentityResolver::make_unreachable`]: every call is unavailable.
    unreachable: Arc<AtomicBool>,
}

impl MemIdentityResolver {
    /// An empty world: every lookup is not found.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage a handle and DID that claim each other, so both lookups confirm.
    pub fn seed_pair(&self, handle: &AtHandle, did: &Did) {
        self.set_handle(handle, Answer::Found(did.clone()));
        self.set_document(did, Answer::Found(Some(handle.clone())));
    }

    /// Stage only `handle`'s record naming `did`, leaving the DID's document
    /// as it is: staged, refused, unavailable, or missing.
    pub fn seed_handle_record(&self, handle: &AtHandle, did: &Did) {
        self.set_handle(handle, Answer::Found(did.clone()));
    }

    /// Stage a one-way claim: `handle` names `did`, but the DID's document does
    /// not claim it back. Gives the DID a document claiming no handle unless one is staged.
    pub fn seed_handle_claim(&self, handle: &AtHandle, did: &Did) {
        self.seed_handle_record(handle, did);
        self.documents
            .lock()
            .expect("MemIdentityResolver documents mutex poisoned")
            .entry(did.clone())
            .or_insert(Answer::Found(None));
    }

    /// Stage `did`'s document, claiming `claimed` (or no handle), without
    /// making that handle name the DID back.
    pub fn seed_document(&self, did: &Did, claimed: Option<&AtHandle>) {
        self.set_document(did, Answer::Found(claimed.cloned()));
    }

    /// Make every lookup of `handle` break a fetch rule.
    pub fn refuse_handle(&self, handle: &AtHandle) {
        self.set_handle(handle, Answer::Refused);
    }

    /// Make every fetch of `did`'s document break a fetch rule.
    pub fn refuse_did(&self, did: &Did) {
        self.set_document(did, Answer::Refused);
    }

    /// Make every lookup of `handle` fail to finish.
    pub fn make_handle_unavailable(&self, handle: &AtHandle) {
        self.set_handle(handle, Answer::Unavailable);
    }

    /// Make every fetch of `did`'s document fail to finish.
    pub fn make_did_unavailable(&self, did: &Did) {
        self.set_document(did, Answer::Unavailable);
    }

    /// Take the whole fake offline: every call is unavailable.
    pub fn make_unreachable(&self) {
        self.unreachable.store(true, Ordering::SeqCst);
    }

    fn set_handle(&self, handle: &AtHandle, answer: Answer<Did>) {
        self.handles
            .lock()
            .expect("MemIdentityResolver handles mutex poisoned")
            .insert(handle.clone(), answer);
    }

    fn set_document(&self, did: &Did, answer: Answer<Option<AtHandle>>) {
        self.documents
            .lock()
            .expect("MemIdentityResolver documents mutex poisoned")
            .insert(did.clone(), answer);
    }

    /// The DID `handle` names, without the back-check; not found when unstaged.
    fn lookup_handle(&self, handle: &AtHandle) -> Result<Did, ResolveError> {
        if self.unreachable.load(Ordering::SeqCst) {
            return Err(unavailable());
        }
        let answer = self
            .handles
            .lock()
            .expect("MemIdentityResolver handles mutex poisoned")
            .get(handle)
            .cloned()
            .ok_or(ResolveError::NotFound)?;
        answer.into_result()
    }

    /// The handle `did`'s document claims; not found when unstaged.
    fn lookup_document(&self, did: &Did) -> Result<Option<AtHandle>, ResolveError> {
        if self.unreachable.load(Ordering::SeqCst) {
            return Err(unavailable());
        }
        let answer = self
            .documents
            .lock()
            .expect("MemIdentityResolver documents mutex poisoned")
            .get(did)
            .cloned()
            .ok_or(ResolveError::NotFound)?;
        answer.into_result()
    }
}

#[async_trait]
impl IdentityResolver for MemIdentityResolver {
    /// Forward lookup, then the back-check against the DID's document.
    async fn resolve_handle(&self, handle: &AtHandle) -> Result<Did, ResolveError> {
        let did = self.lookup_handle(handle)?;
        let claimed = self.lookup_document(&did)?;
        if claimed.as_ref() != Some(handle) {
            return Err(ResolveError::NotConfirmed);
        }
        Ok(did)
    }

    /// The document's claim, kept only when that handle names the DID back; a
    /// back-check that could not finish is an error, any other failure `None`.
    async fn resolve_did(&self, did: &Did) -> Result<Option<AtHandle>, ResolveError> {
        let Some(claimed) = self.lookup_document(did)? else {
            return Ok(None);
        };
        match self.lookup_handle(&claimed) {
            Ok(named) if named == *did => Ok(Some(claimed)),
            Err(unavailable @ ResolveError::Unavailable(_)) => Err(unavailable),
            Ok(_) | Err(_) => Ok(None),
        }
    }
}
