use crate::use_case;
use domain::elements::{
    commission::{CommissionId, FileDownload, FileKey},
    user::UserId,
};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, den_refusal, files::Files},
    den::access::{ServedNode, may_open},
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
    /// Retrieves a file entry's metadata and a live reader over its bytes, once den's check
    /// opens the commission (else `NotAMember`) and then the file under it (else
    /// `FileNotFound`, whether the key is absent or another commission's). Before both pass,
    /// nothing is read by key but the names of the files in the commission's own list.
    #[use_case]
    pub async fn download(&self, #[ports] ports: &Ports, query: Query) -> CommissionResult<Output> {
        let Query {
            actor_id,
            commission_id,
            file_id,
        } = query;

        may_open(ports, &actor_id, ServedNode::Commission(commission_id))
            .await
            .map_err(|refusal| den_refusal(refusal, CommissionError::NotAMember))?;
        let opens = may_open(ports, &actor_id, ServedNode::File(commission_id, file_id))
            .await
            .map_err(|refusal| den_refusal(refusal, CommissionError::FileNotFound))?;
        let &ServedNode::File(commission_id, file_id) = opens.node() else {
            return Err(CommissionError::FileNotFound);
        };

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

#[cfg(test)]
mod tests;
