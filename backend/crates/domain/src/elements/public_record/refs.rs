use cid::Cid;

use super::AtUri;

/// A strong reference to a record: its [`AtUri`] paired with the content-hash
/// [`Cid`] of the exact revision pointed at — a pointer that a change to the
/// target invalidates. Mirrors `com.atproto.repo.strongRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrongRef {
    /// The referenced record's address.
    pub uri: AtUri,
    /// The content hash of the referenced revision.
    pub cid: Cid,
}

/// The address + content hash a write returns: where the record landed and the
/// [`Cid`] of the revision just written. Distinct from [`StrongRef`] because it
/// names a write result, not a reference to another record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRef {
    /// Where the record was written.
    pub uri: AtUri,
    /// The content hash of the written revision.
    pub cid: Cid,
}

/// A reference to an uploaded blob: its content-address [`Cid`] plus the mime
/// type and byte size the repo recorded. Byte-identical blobs share a ref
/// network-wide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    /// The blob's content-address.
    pub cid: Cid,
    /// The mime type the repo stored for the blob.
    pub mime_type: String,
    /// The blob's size in bytes.
    pub size: u64,
}

impl BlobRef {
    /// The blob's content-addressed identity — the
    /// [`BlobId`](crate::elements::blob::BlobId) the rest of the domain uses.
    pub fn id(&self) -> crate::elements::blob::BlobId {
        crate::elements::blob::BlobId::new(self.cid)
    }
}
