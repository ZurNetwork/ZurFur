use super::WorkflowId;
use crate::elements::text::StoredText;

/// One Workflow as an Account's listing reads it: its id and stored name.
/// Its columns and cards are not part of this read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSummary {
    /// The Workflow's id.
    pub id: WorkflowId,
    /// The name as stored, not re-checked on load.
    pub name: StoredText,
}
