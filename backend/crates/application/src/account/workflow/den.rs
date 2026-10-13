//! Commission's arm of the Den for Workflows: an Account's `workflows` folder and the
//! Workflows inside it.

use domain::elements::{account::AccountId, workflow::WorkflowId};

use crate::den::{
    DenContext, DenMount, DenType, Level, Node, NodeState, Place, Segment, TopLevel,
    names::{self, WORKFLOWS},
};

/// The places this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowPlace {
    /// An Account's `workflows` folder.
    #[allow(
        dead_code,
        reason = "built by the identity arms, which answer the Account's folders"
    )]
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

/// The Account's Workflows, each Private and live, named by its stored name. A Workflow's
/// stored visibility is a default for new commissions, never its own level.
async fn workflows(
    context: &DenContext<'_>,
    account: &AccountId,
    only: Option<&Segment>,
) -> anyhow::Result<Vec<Node>> {
    let summaries = context.ports.workflows.list_for_account(account).await?;
    let nodes = summaries
        .into_iter()
        .filter(|summary| {
            only.is_none_or(|key| names::id_segment(summary.id).as_ref() == Some(key))
        })
        .filter_map(|summary| {
            let node = Node {
                place: Place::Workflow(WorkflowPlace::Workflow(account.clone(), summary.id)),
                name: names::stored_name(summary.name.as_str())?,
                level: Level::Private,
                lifts: Vec::new(),
                state: NodeState::Live,
            };
            Some(node)
        })
        .collect();
    Ok(nodes)
}

/// The Workflow at `key` in an Account's `workflows` folder, from that Account's own list.
/// A Workflow holds nothing the Den shows yet.
pub(crate) async fn child(
    context: &DenContext<'_>,
    place: &WorkflowPlace,
    key: &Segment,
) -> anyhow::Result<Option<Node>> {
    match place {
        WorkflowPlace::Workflows(account) => {
            let found = workflows(context, account, Some(key)).await?;
            Ok(found.into_iter().next())
        }
        WorkflowPlace::Workflow(..) => Ok(None),
    }
}

/// Every Workflow in an Account's `workflows` folder; a Workflow lists nothing yet.
pub(crate) async fn children(
    context: &DenContext<'_>,
    place: &WorkflowPlace,
) -> anyhow::Result<Vec<Node>> {
    match place {
        WorkflowPlace::Workflows(account) => workflows(context, account, None).await,
        WorkflowPlace::Workflow(..) => Ok(Vec::new()),
    }
}

/// The viewer's mounts in `place`: none, since Workflows are reached by traversal.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &WorkflowPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests;
