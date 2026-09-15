use async_trait::async_trait;

use super::errors::PublicRecordsError;
use crate::elements::did::Did;
use crate::elements::public_record::{AtUri, BlobRef, PublicRecord, RecordRef};

/// The **write** surface of the public data boundary: create / put / delete /
/// read a record, and upload a blob, in the acting identity's atproto repo.
/// Auth-agnostic by construction — it speaks [`Did`] and domain records only,
/// never a credential, so the PDS credential stays inside its adapter. Every call
/// is its own retryable step, never fused with a private-store [`UnitOfWork`].
#[async_trait]
pub trait PublicRecords: Send + Sync {
    /// Create a new record in `repo`'s collection (the NSID follows the record
    /// variant), letting the repo mint the rkey. Returns where it landed. A `repo`
    /// the adapter cannot act in is a [`PublicRecordsError::Rejected`].
    async fn create_record(
        &self,
        repo: &Did,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError>;

    /// Upsert the record at `uri` (create-or-overwrite at that exact key).
    /// Returns the new [`RecordRef`]. Idempotent for identical content.
    async fn put_record(
        &self,
        uri: &AtUri,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError>;

    /// Delete the record at `uri`. Deleting an absent record is a no-op, not an
    /// error.
    async fn delete_record(&self, uri: &AtUri) -> Result<(), PublicRecordsError>;

    /// Read the record at `uri` back as a typed [`PublicRecord`], or
    /// [`PublicRecordsError::NotFound`].
    async fn get_record(&self, uri: &AtUri) -> Result<PublicRecord, PublicRecordsError>;

    /// Upload blob bytes to the acting identity's repo, returning the
    /// content-addressed [`BlobRef`] a record can embed. Byte-identical uploads
    /// address to the same CID.
    async fn upload_blob(
        &self,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<BlobRef, PublicRecordsError>;
}
