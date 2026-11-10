use crate::use_case;
use domain::elements::{
    commission::{CommissionId, FileDownload, FileKey},
    user::UserId,
};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, den_refusal, files::Files},
    den::{
        DenName,
        access::{ServedNode, may_open},
        names,
    },
};

pub struct Query {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub file_id: FileKey,
}

/// The file entry, and the name it is saved under.
pub struct Output {
    pub result: FileDownload,
    /// The name to save the file under: its Den name, the stored name when the name rule
    /// accepts it, else `Unnamed`.
    pub save_name: DenName,
}

impl Files<'_> {
    /// Retrieves a file entry's metadata, a live reader over its bytes and its save name (its
    /// Den name), once den's check opens the commission (else `NotAMember`) and then the file
    /// under it (else `FileNotFound`, whether the key is absent or another commission's). Before
    /// both pass, nothing is read by key but the names of the files in the commission's own list.
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
        // The Den has no node for a file it can't name, so neither has the download.
        let save_name = names::stored_name(result.metadata.filename.as_str())
            .map(DenName::from)
            .ok_or(CommissionError::FileNotFound)?;

        Ok(Output { result, save_name })
    }
}

#[cfg(test)]
mod tests;
