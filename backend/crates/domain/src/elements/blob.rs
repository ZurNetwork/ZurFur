//! [`BlobId`] — the identity of a Blob. **Stub.**
//!
//! A Blob is the raw binary payload a Post points at. Blobs are
//! content-addressed, so a Blob's identity IS the hash of its bytes. Only the id
//! type exists so far. (DESIGN 9994275)

use cid::Cid;

/// The content-addressed identity of a Blob: its [`Cid`]. The same bytes always
/// yield the same `BlobId`, and the id changes if any byte changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobId(Cid);

impl BlobId {
    /// Wrap a blob's content-address [`Cid`], computed by the store that holds
    /// the bytes — never invented by the domain.
    pub fn new(cid: Cid) -> Self {
        Self(cid)
    }

    /// The underlying content-address.
    pub fn cid(&self) -> &Cid {
        &self.0
    }
}
