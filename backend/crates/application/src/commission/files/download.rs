use domain::elements::{
    commission::{CommissionId, FileDownload, FileKey},
    user::UserId,
};

use crate::{
    commission::{CommissionError, CommissionResult, files::Files},
    ports::WithPorts,
};

pub struct Query {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub file_id: FileKey,
}

pub struct Output {
    pub result: FileDownload,
}

impl Files<'_> {
    /// Retrieves a file entry's metadata and a live reader over its bytes.
    /// Participant-gated; `FileNotFound` for a key that is absent, or belongs to
    /// another commission — the same answer either way, so retrieval is never a
    /// cross-commission existence oracle. A blob missing under an existing link
    /// is an internal inconsistency, surfaced as `Infrastructure`.
    pub async fn download(&self, query: Query) -> CommissionResult<Output> {
        let Query {
            actor_id,
            commission_id,
            file_id,
        } = query;
        let ports = self.ports();

        if !ports
            .commissions
            .is_participant(&commission_id, &actor_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }

        ports
            .commissions
            .find_file(&commission_id, file_id)
            .await?
            .ok_or(CommissionError::FileNotFound)?;

        let result = ports
            .files
            .get(file_id)
            .await
            .map_err(CommissionError::Infrastructure)?
            .ok_or(CommissionError::FileBlobMissing)?;

        Ok(Output { result })
    }
}
