//! Commission's arm of the Den for Workflows: an Account's `workflows` folder and the
//! Workflows inside it.

use domain::elements::{account::AccountId, workflow::WorkflowId};

use crate::den::{
    DenContext, DenMount, DenType, Node, Segment, TopLevel,
    names::{self, WORKFLOWS},
};

/// The places this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "the commission arms build these once they serve Workflow nodes"
)]
pub(crate) enum WorkflowPlace {
    /// An Account's `workflows` folder.
    Workflows(AccountId),
    /// One Workflow in it.
    Workflow(AccountId, WorkflowId),
}

impl WorkflowPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            WorkflowPlace::Workflows(_) => DenType::AccountWorkflows,
            WorkflowPlace::Workflow(..) => DenType::Workflow,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            WorkflowPlace::Workflows(_) => names::fixed_segment(WORKFLOWS),
            WorkflowPlace::Workflow(_, workflow) => names::id_segment(workflow),
        }
    }

    /// The top-level directory the node sits directly under; none does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            WorkflowPlace::Workflows(_) | WorkflowPlace::Workflow(..) => None,
        }
    }
}

/// The node at `key` in the folder `place`. Not served yet: always absent.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &WorkflowPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of the folder `place`. Not served yet: always empty.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &WorkflowPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in the folder `place`. Not served yet: always empty.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &WorkflowPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}
