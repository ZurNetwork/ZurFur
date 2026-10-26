use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionFile, CommissionId, CommissionRole, FileKey, FileName, NewChangelogEntry,
            OpenKind,
        },
        user::UserId,
    },
};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, files::Files, require},
    den::DenName,
};

/// The acting Participant, the target commission, and the wire-optional
/// filename and content type declared alongside the bytes.
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

/// The new file entry's opaque key.
#[derive(Debug)]
pub struct Output {
    pub id: FileKey,
}

impl Files<'_> {
    /// Uploads a file entry: any Participant, any time, never a status side
    /// effect. Authorizes, then checks the filename as it will be stored (a
    /// dotfile is refused), before a byte of `content` is read; caps it at
    /// `max_upload_bytes`, and deletes the orphaned blob before refusing.
    /// Commits the [`CommissionFile`] link and its changelog entry atomically.
    #[use_case]
    pub async fn upload(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        content: impl AsyncRead + Send + Unpin,
        max_upload_bytes: u64,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            filename,
            content_type,
        } = cmd;
        require(
            ports,
            &commission_id,
            &actor_id,
            CommissionRole::Participant,
        )
        .await?;

        let filename = FileName::try_from(filename.unwrap_or_default())
            .map_err(CommissionError::InvalidFileName)?;
        // Stored as the Den's name rule keeps it, without the characters the rule strips;
        // checked again, so a name that only starts with a dot once stripped is refused too.
        let den_name =
            DenName::try_from(filename.as_str()).map_err(CommissionError::InvalidName)?;
        let filename =
            FileName::try_from(den_name.as_ref()).map_err(CommissionError::InvalidFileName)?;
        let content_type = content_type.unwrap_or_default();
        let key = FileKey::generate();

        let mut capped = content.take(max_upload_bytes + 1);
        let written = ports
            .files
            .put(key, &filename, &content_type, &mut capped)
            .await?;

        if written > max_upload_bytes {
            if let Err(err) = ports.files.delete(key).await {
                tracing::warn!(
                    error = ?err,
                    file_id = %key,
                    "failed to delete an over-cap upload's orphaned blob",
                );
            }
            return Err(CommissionError::FileTooLarge);
        }
        if written == 0 {
            if let Err(err) = ports.files.delete(key).await {
                tracing::warn!(
                    error = ?err,
                    file_id = %key,
                    "failed to delete an empty upload's orphaned blob",
                );
            }
            return Err(CommissionError::FileEmpty);
        }

        let entry: NewChangelogEntry = NewChangelogEntry::event(
            commission_id,
            OpenKind::FileAdded,
            actor_id.clone(),
            json!({
                "file_id": *key.as_ref(),
                "filename": filename.as_str(),
                "content_type": content_type,
                "byte_size": written,
            }),
            now,
        )
        .into();
        let file = CommissionFile {
            id: key,
            commission_id,
            uploaded_by: actor_id,
            created_at: now,
        };
        let uow = uow.open().await?;
        uow.commissions().add_file(&file).await?;
        uow.changelog().append(&entry).await?;
        Ok(Output { id: key })
    }
}
