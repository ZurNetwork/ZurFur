//! [`BlobId`] — the identity of a Blob. **Stub.**
//!
//! A Blob is the raw binary payload a Post points at — the actual bytes of an
//! image, video, audio file, document, or archive (DESIGN/Blob; see also
//! DESIGN/"Blobs, PDS & Private Storage"). The Post is the metadata; the Blob is
//! the content. Blobs are content-addressed, so a Blob's identity *is* the hash
//! of its bytes — hence [`BlobId`] wraps a [`Cid`]. Only the id type exists so
//! far; the Blob entity itself is not modelled here yet.

use cid::Cid;

/// The content-addressed identity of a Blob: its [`Cid`] (IPFS-style CID).
///
/// Stub: identity only. Because it is the content hash, the same bytes always
/// yield the same `BlobId`, and the id changes if any byte changes. A
/// [`BlobRef`](crate::elements::public_record::BlobRef) — the public-boundary
/// reference an embed carries — is this same content-address plus the mime/size
/// the repo recorded ([`BlobRef::id`](crate::elements::public_record::BlobRef::id)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobId(Cid);

impl BlobId {
    /// Wrap a blob's content-address [`Cid`]. The content-address is computed by
    /// the store that holds the bytes (the PDS/blob store), never invented by the
    /// domain — this only names one the caller already holds.
    pub fn new(cid: Cid) -> Self {
        Self(cid)
    }

    /// The underlying content-address.
    pub fn cid(&self) -> &Cid {
        &self.0
    }
}
