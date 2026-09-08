use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, CommissionFile, CommissionId, FileKey, FileName, NewChangelogEntry,
        },
        user::UserId,
    },
};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{
    commission::{CommissionError, CommissionResult, files::Files},
    ports::WithPorts,
};

/// `upload`'s input: the acting Participant, the target commission, and the
/// wire-optional filename/content-type declared alongside the bytes.
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

/// `upload`'s output: the newly minted file entry's opaque key.
#[derive(Debug)]
pub struct Output {
    pub id: FileKey,
}

impl Files<'_> {
    /// Uploads a file entry: any Participant, any time, never a status side
    /// effect. Authorizes **before** a byte of `content` is read. `content` is
    /// capped at `max_upload_bytes` (+1, to prove an over-cap stream is over
    /// without buffering the whole overage); the blob write runs **before** the
    /// transaction (bytes cannot ride a Postgres unit of work), so a rejected
    /// upload's orphaned blob is deleted before answering. Commits the
    /// [`CommissionFile`] link and the `file_added` changelog entry atomically.
    pub async fn upload(
        &self,
        cmd: Command,
        content: impl AsyncRead + Send + Unpin,
        max_upload_bytes: u64,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            filename,
            content_type,
        } = cmd;
        if !ports
            .commissions
            .is_participant(&commission_id, &actor_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }

        let filename = FileName::try_new(filename.unwrap_or_default())
            .map_err(CommissionError::InvalidFileName)?;
        let content_type = content_type.unwrap_or_default();
        let key = FileKey::generate();

        let mut capped = content.take(max_upload_bytes + 1);
        let written = ports
            .files
            .put(key, &filename, &content_type, &mut capped)
            .await
            .map_err(CommissionError::Infrastructure)?;

        if written > max_upload_bytes {
            if let Err(err) = ports.files.delete(key).await {
                tracing::warn!(
                    error = ?err,
                    file_id = %*key,
                    "failed to delete an over-cap upload's orphaned blob",
                );
            }
            return Err(CommissionError::FileTooLarge);
        }
        if written == 0 {
            if let Err(err) = ports.files.delete(key).await {
                tracing::warn!(
                    error = ?err,
                    file_id = %*key,
                    "failed to delete an empty upload's orphaned blob",
                );
            }
            return Err(CommissionError::FileEmpty);
        }

        let entry = NewChangelogEntry::event(
            commission_id,
            ChangelogEntryKind::FileAdded,
            actor_id.clone(),
            json!({
                "file_id": *key,
                "filename": filename.as_str(),
                "content_type": content_type,
                "byte_size": written,
            }),
            now,
        );
        let file = CommissionFile {
            id: key,
            commission_id,
            uploaded_by: actor_id,
            created_at: now,
        };
        let mut uow = self.ports().database.begin().await?;
        uow.commissions().add_file(&file).await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output { id: key })
    }
}
