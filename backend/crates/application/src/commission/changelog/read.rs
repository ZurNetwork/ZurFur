use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId},
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, changelog::Changelog, den_refusal},
    den::access::{ServedNode, may_open},
};

pub struct Query {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

pub struct ChangelogEntry {
    pub seq: i64,
    pub kind: ChangelogEntryKind,
    pub actor_id: Option<UserId>,
    pub payload: serde_json::Value,
    pub note: Option<String>,
    pub created_at: DateTimeUtc,
}
pub struct Output {
    pub entries: Vec<ChangelogEntry>,
}

impl Changelog<'_> {
    /// Reads the commission's changelog in stream order, ascending `seq`, once den's check
    /// opens its `changelog`. Everyone it doesn't open to gets `NotAMember`, which the drivers
    /// render byte-identically to an absent commission's `404`.
    #[use_case]
    pub async fn read(&self, #[ports] ports: &Ports, query: Query) -> CommissionResult<Output> {
        let Query {
            actor_id,
            commission_id,
        } = query;
        let opens = may_open(ports, &actor_id, ServedNode::Changelog(commission_id))
            .await
            .map_err(|refusal| den_refusal(refusal, CommissionError::NotAMember))?;
        let &ServedNode::Changelog(commission_id) = opens.node() else {
            return Err(CommissionError::NotAMember);
        };

        Ok(Output {
            entries: ports
                .changelog
                .entries(&commission_id)
                .await?
                .into_iter()
                // `seq` is the STORE's ordering key, carried through untouched:
                // monotonic per stream but not gapless, never renumbered here.
                .map(|entry| ChangelogEntry {
                    seq: entry.seq,
                    actor_id: entry.actor_id,
                    kind: entry.kind,
                    payload: entry.payload,
                    note: entry.note,
                    created_at: entry.created_at,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests;
