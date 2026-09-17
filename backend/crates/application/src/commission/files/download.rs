use domain::elements::{
    commission::{CommissionId, FileDownload, FileKey},
    user::UserId,
};
use macros::use_case;

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, files::Files},
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
    /// Participant-gated; `FileNotFound` whether the key is absent or belongs
    /// to another commission, so retrieval is never an existence oracle.
    #[use_case]
    pub async fn download(&self, #[ports] ports: &Ports, query: Query) -> CommissionResult<Output> {
        let Query {
            actor_id,
            commission_id,
            file_id,
        } = query;

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
            .await?
            .ok_or(CommissionError::FileBlobMissing)?;

        Ok(Output { result })
    }
}
