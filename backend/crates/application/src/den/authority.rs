//! Who may write where: the pool's early verdict, before any unit opens, and the grant the
//! locked read makes inside the unit. The grant is the write's only authority; nothing else in
//! the write names a thing to act on.

use std::fmt;

use domain::{
    elements::{
        commission::{CommissionId, CommissionRole},
        level::Level,
        node::NodeId,
        user::UserId,
    },
    ports::{New, Unit},
};

use crate::{
    Ports,
    commission::den::{CommissionPlace, Verdict, pool_verdict, verdict},
    den::{
        Place, TopLevel,
        act::{ActShape, ActSpec, Anchor},
        write::{DenWriteError, RefusalClass, broken},
    },
};

/// What a grant leaves its actor to write: an existing commission in a role, or a new one
/// under an id the den minted.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::den) enum GrantKind {
    /// An existing commission, held in `role`, at the level the locked read answered, bound
    /// to the node that read joined.
    Existing {
        /// The commission.
        commission: CommissionId,
        /// The actor's role on it, from the locked read.
        role: CommissionRole,
        /// Its own level, from the locked read.
        level: Level,
        /// The node it points at, from the locked read: the only node its walk may land on.
        node: NodeId,
    },
    /// A commission not made yet, under the id the den minted.
    Create {
        /// The minted id.
        commission: New<CommissionId>,
    },
}

/// Leave for one write: who acts and what on. Its fields are private, it is neither `Clone`
/// nor `Default`, and only this module makes one.
pub(crate) struct Grant {
    actor: UserId,
    kind: GrantKind,
}

impl Grant {
    /// The grant's one constructor.
    fn granted(actor: &UserId, kind: GrantKind) -> Self {
        Self {
            actor: actor.clone(),
            kind,
        }
    }

    /// A grant of `kind` to `actor`, for the tests that bind a write to its grant.
    #[cfg(test)]
    pub(in crate::den) fn for_test(actor: &UserId, kind: GrantKind) -> Self {
        Self::granted(actor, kind)
    }

    /// Who acts.
    pub(in crate::den) fn actor(&self) -> &UserId {
        &self.actor
    }

    /// What the grant leaves its actor to write.
    pub(in crate::den) fn kind(&self) -> &GrantKind {
        &self.kind
    }

    /// The existing commission, when the actor holds `role` or above on it.
    pub(in crate::den) fn existing_at_least(&self, role: CommissionRole) -> Option<CommissionId> {
        match self.kind {
            GrantKind::Existing {
                commission,
                role: held,
                ..
            } => (held >= role).then_some(commission),
            GrantKind::Create { .. } => None,
        }
    }

    /// The node an existing commission's grant is bound to; `None` for a create grant.
    pub(in crate::den) fn bound_node(&self) -> Option<NodeId> {
        match self.kind {
            GrantKind::Existing { node, .. } => Some(node),
            GrantKind::Create { .. } => None,
        }
    }

    /// The id the den minted, for a create grant.
    pub(in crate::den) fn minted(&self) -> Option<New<CommissionId>> {
        match self.kind {
            GrantKind::Create { commission } => Some(commission),
            GrantKind::Existing { .. } => None,
        }
    }
}

impl fmt::Debug for Grant {
    /// Its kind alone; never its actor, whose id is a DID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Grant")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
tokio::task_local! {
    /// The id the next mint answers in this task, for the tests that need a known one.
    pub(in crate::den) static MINT_AS: CommissionId;
}

/// A fresh v7 commission id.
fn mint() -> CommissionId {
    #[cfg(test)]
    if let Ok(planted) = MINT_AS.try_with(|planted| *planted) {
        return planted;
    }
    CommissionId::mint()
}

/// The commission an anchor's place stands for; any other place has no arm in the den's
/// write yet and is the one not-found.
fn commission_of(place: &Place) -> Result<CommissionId, DenWriteError> {
    let Place::Commission(CommissionPlace::Commission(commission)) = place else {
        return Err(DenWriteError::NotFound);
    };
    Ok(*commission)
}

/// The role a thing anchor's act needs; an act that needs none, a create's, fails closed.
fn needed(spec: &ActSpec) -> Result<CommissionRole, DenWriteError> {
    spec.needs
        .ok_or(DenWriteError::Refused(RefusalClass::WrongGrant))
}

/// The rule for a top-level anchor: only a create carries one, and only `/commission` has an
/// arm for it.
fn top_level_create(top_level: TopLevel, shape: &ActShape) -> Result<(), DenWriteError> {
    if !matches!(shape, ActShape::Create { .. }) {
        return Err(DenWriteError::Refused(RefusalClass::WrongGrant));
    }
    if top_level != TopLevel::Commission {
        return Err(DenWriteError::NotFound);
    }
    Ok(())
}

/// A verdict as the write's answer: granted, or the one not-found, or forbidden.
fn answer(verdict: Verdict) -> Result<crate::commission::den::StandingFacts, DenWriteError> {
    match verdict {
        Verdict::NotFound => Err(DenWriteError::NotFound),
        Verdict::Forbidden => Err(DenWriteError::Forbidden),
        Verdict::Granted(facts) => Ok(facts),
    }
}

/// The pool's verdict on `spec` for `actor`, before any unit opens: a top-level anchor answers
/// by rule alone, and a thing's by one standing statement, once its act names a role.
pub(in crate::den) async fn admit(
    ports: &Ports,
    spec: &ActSpec,
    actor: &UserId,
) -> Result<(), DenWriteError> {
    match &spec.anchor {
        Anchor::TopLevel(top_level) => top_level_create(*top_level, &spec.shape),
        Anchor::Thing(place) => {
            let needs = needed(spec)?;
            let commission = commission_of(place)?;
            let verdict = pool_verdict(ports, &commission, actor, needs).await?;
            answer(verdict)?;
            Ok(())
        }
    }
}

/// The grant for `spec`, made inside the unit from the locked standing, under the same rules as
/// [`admit`]: a standing lost since the pool read answers as the pool would have. A create's
/// grant names an id minted here, never one from the act.
pub(in crate::den) async fn grant(
    unit: Unit<'_>,
    spec: &ActSpec,
    actor: &UserId,
) -> Result<Grant, DenWriteError> {
    match &spec.anchor {
        Anchor::TopLevel(top_level) => {
            top_level_create(*top_level, &spec.shape)?;
            let kind = GrantKind::Create {
                commission: New::from(mint()),
            };
            Ok(Grant::granted(actor, kind))
        }
        Anchor::Thing(place) => {
            let needs = needed(spec)?;
            let commission = commission_of(place)?;
            let locked = unit
                .commissions()
                .standing_for_update(&commission, actor)
                .await?;
            let standing = locked.map(|locked| locked.standing);
            let facts = answer(verdict(standing, needs))?;
            let Some(locked) = locked else {
                return Err(broken("a granted standing holds no node"));
            };
            let kind = GrantKind::Existing {
                commission,
                role: facts.role,
                level: facts.level,
                node: locked.node,
            };
            Ok(Grant::granted(actor, kind))
        }
    }
}

#[cfg(test)]
mod tests;
