//! Who may write where: the pool's early verdict, before any unit opens, and the grant the
//! locked read makes inside the unit. The grant is the write's only authority; nothing else in
//! the write names a thing to act on.

use std::fmt;

use domain::{
    elements::{
        commission::{CommissionId, CommissionRole, CommissionState, LateFacts, LateKinds},
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
        act::{ActShape, ActSpec, Anchor, Needs},
        write::{Actor, DenWriteError, RefusalClass, broken},
    },
};

/// What a grant leaves its actor to write: an existing commission in a role, with the facts
/// its rules read; a new one under an id the den minted; or, to the system alone, an existing
/// commission with the facts its Late rule reads.
#[derive(Debug, PartialEq)]
pub(in crate::den) enum GrantKind {
    /// An existing commission, held in `role`, at the level the locked read answered, bound
    /// to the node that read joined, with its stored state as the unit read it under the lock.
    Existing {
        /// The commission.
        commission: CommissionId,
        /// The actor's role on it, from the locked read.
        role: CommissionRole,
        /// Its own level, from the locked read.
        level: Level,
        /// The node it points at, from the locked read: the only node its walk may land on.
        node: NodeId,
        /// Its stored state, read in the unit after the lock: what each act's rule reads.
        state: CommissionState,
    },
    /// A commission not made yet, under the id the den minted.
    Create {
        /// The minted id.
        commission: New<CommissionId>,
    },
    /// An existing commission the system acts on, with no role: bound to the node the system's
    /// locked read answered, with the facts its Late rule reads, read in the unit after the lock.
    System {
        /// The commission.
        commission: CommissionId,
        /// Its own level, from the locked read.
        level: Level,
        /// The node it points at, from the locked read: the only node its walk may land on.
        node: NodeId,
        /// Its stored state and whether a `late` entry is still owed, read after the lock.
        facts: LateFacts,
    },
}

/// Leave for one write: who acts and what on. Its fields are private, it is neither `Clone`
/// nor `Default`, and only this module makes one.
pub(crate) struct Grant {
    actor: Actor,
    kind: GrantKind,
}

impl Grant {
    /// The grant's one constructor.
    fn granted(actor: Actor, kind: GrantKind) -> Self {
        Self { actor, kind }
    }

    /// A grant of `kind` to the user `actor`, for the tests that bind a write to its grant.
    #[cfg(test)]
    pub(in crate::den) fn for_test(actor: &UserId, kind: GrantKind) -> Self {
        Self::granted(Actor::User(actor.clone()), kind)
    }

    /// A grant of `kind` to the system, for the tests that bind the system's act to its grant.
    #[cfg(test)]
    pub(in crate::den) fn for_system_test(kind: GrantKind) -> Self {
        let token = crate::commission::sweep::SweepToken::for_test();
        Self::granted(Actor::System(token), kind)
    }

    /// Who acts.
    pub(in crate::den) fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The user who acts, or `None` when the system does.
    pub(in crate::den) fn user(&self) -> Option<&UserId> {
        self.actor.user()
    }

    /// What the grant leaves its actor to write.
    pub(in crate::den) fn kind(&self) -> &GrantKind {
        &self.kind
    }

    /// The existing commission, when the actor holds `role` or above on it; never the system's.
    pub(in crate::den) fn existing_at_least(&self, role: CommissionRole) -> Option<CommissionId> {
        match self.kind {
            GrantKind::Existing {
                commission,
                role: held,
                ..
            } => (held >= role).then_some(commission),
            GrantKind::Create { .. } | GrantKind::System { .. } => None,
        }
    }

    /// The existing commission's stored state, when the actor holds `role` or above on it;
    /// never the system's.
    pub(in crate::den) fn state_at_least(&self, role: CommissionRole) -> Option<&CommissionState> {
        match &self.kind {
            GrantKind::Existing {
                role: held, state, ..
            } => (*held >= role).then_some(state),
            GrantKind::Create { .. } | GrantKind::System { .. } => None,
        }
    }

    /// The commission the system's grant leaves it, for the system's act alone.
    pub(in crate::den) fn system(&self) -> Option<CommissionId> {
        match self.kind {
            GrantKind::System { commission, .. } => Some(commission),
            GrantKind::Existing { .. } | GrantKind::Create { .. } => None,
        }
    }

    /// The facts the system's grant read for its Late rule, for the system's act alone.
    pub(in crate::den) fn late_facts(&self) -> Option<&LateFacts> {
        match &self.kind {
            GrantKind::System { facts, .. } => Some(facts),
            GrantKind::Existing { .. } | GrantKind::Create { .. } => None,
        }
    }

    /// The existing commission the grant acts on, a user's or the system's; `None` for a create.
    pub(in crate::den) fn acted_on(&self) -> Option<CommissionId> {
        match self.kind {
            GrantKind::Existing { commission, .. } | GrantKind::System { commission, .. } => {
                Some(commission)
            }
            GrantKind::Create { .. } => None,
        }
    }

    /// The node an existing commission's grant is bound to; `None` for a create grant.
    pub(in crate::den) fn bound_node(&self) -> Option<NodeId> {
        match self.kind {
            GrantKind::Existing { node, .. } | GrantKind::System { node, .. } => Some(node),
            GrantKind::Create { .. } => None,
        }
    }

    /// The id the den minted, for a create grant.
    pub(in crate::den) fn minted(&self) -> Option<New<CommissionId>> {
        match self.kind {
            GrantKind::Create { commission } => Some(commission),
            GrantKind::Existing { .. } | GrantKind::System { .. } => None,
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

/// The refusal of an actor the act doesn't take: a user act's for the system, the system's for
/// a user, and a create's on a thing for anyone.
fn wrong_actor<T>() -> Result<T, DenWriteError> {
    Err(DenWriteError::Refused(RefusalClass::WrongGrant))
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
/// by rule alone, and only to a user; a thing's by one standing statement, once its act names a
/// role and a user acts; the system's act on a thing by rule alone, and only to the system.
pub(in crate::den) async fn admit(
    ports: &Ports,
    spec: &ActSpec,
    actor: &Actor,
) -> Result<(), DenWriteError> {
    match &spec.anchor {
        Anchor::TopLevel(top_level) => {
            top_level_create(*top_level, &spec.shape)?;
            actor.user().map_or_else(wrong_actor, |_| Ok(()))
        }
        Anchor::Thing(place) => match (spec.needs, actor) {
            (Needs::Role(needs), Actor::User(user)) => {
                let commission = commission_of(place)?;
                let verdict = pool_verdict(ports, &commission, user, needs).await?;
                answer(verdict)?;
                Ok(())
            }
            (Needs::System, Actor::System(_)) => {
                commission_of(place)?;
                Ok(())
            }
            (Needs::Create | Needs::Role(_), Actor::System(_))
            | (Needs::Create | Needs::System, Actor::User(_)) => wrong_actor(),
        },
    }
}

/// The grant for `spec`, made inside the unit under [`admit`]'s rules: a user's from the locked
/// standing, the system's from its own locked read, each carrying the state read after the
/// lock; a standing lost since the pool read answers as the pool would have. A create's grant
/// names an id minted here, never one from the act.
pub(in crate::den) async fn grant(
    unit: Unit<'_>,
    spec: &ActSpec,
    actor: Actor,
) -> Result<Grant, DenWriteError> {
    match &spec.anchor {
        Anchor::TopLevel(top_level) => {
            top_level_create(*top_level, &spec.shape)?;
            if actor.user().is_none() {
                return wrong_actor();
            }
            let kind = GrantKind::Create {
                commission: New::from(mint()),
            };
            Ok(Grant::granted(actor, kind))
        }
        Anchor::Thing(place) => match (spec.needs, actor) {
            (Needs::Role(needs), Actor::User(user)) => {
                let commission = commission_of(place)?;
                let kind = user_grant(unit, commission, &user, needs).await?;
                Ok(Grant::granted(Actor::User(user), kind))
            }
            (Needs::System, Actor::System(token)) => {
                let commission = commission_of(place)?;
                let kind = system_grant(unit, commission).await?;
                Ok(Grant::granted(Actor::System(token), kind))
            }
            (Needs::Create | Needs::Role(_), Actor::System(_))
            | (Needs::Create | Needs::System, Actor::User(_)) => wrong_actor(),
        },
    }
}

/// A user's grant on `commission`, from the locked standing, when they hold `needs` or above.
async fn user_grant(
    unit: Unit<'_>,
    commission: CommissionId,
    user: &UserId,
    needs: CommissionRole,
) -> Result<GrantKind, DenWriteError> {
    let locked = unit
        .commissions()
        .standing_for_update(&commission, user)
        .await?;
    let standing = locked.map(|locked| locked.standing);
    let facts = answer(verdict(standing, needs))?;
    let Some(locked) = locked else {
        return Err(broken("a granted standing holds no node"));
    };
    let stored = unit
        .commissions()
        .find(&commission)
        .await?
        .ok_or_else(|| broken("a granted commission has no row"))?;
    let kind = GrantKind::Existing {
        commission,
        role: facts.role,
        level: facts.level,
        node: locked.node,
        state: CommissionState::from(&stored),
    };
    Ok(kind)
}

/// The system's grant on `commission`, from its own locked read, which reads no viewer: then
/// the row and whether a `late` entry is still owed, each read after the lock. A commission
/// gone since the scan is the one not-found.
async fn system_grant(
    unit: Unit<'_>,
    commission: CommissionId,
) -> Result<GrantKind, DenWriteError> {
    let locked = unit
        .commissions()
        .lock_for_system(&commission)
        .await?
        .ok_or(DenWriteError::NotFound)?;
    let stored = unit
        .commissions()
        .find(&commission)
        .await?
        .ok_or_else(|| broken("a locked commission has no row"))?;
    let late_kinds = LateKinds::default();
    let owed = unit
        .commissions()
        .late_owed(&commission, &late_kinds)
        .await?;
    let facts = LateFacts {
        state: CommissionState::from(&stored),
        owed,
    };
    let kind = GrantKind::System {
        commission,
        level: locked.level,
        node: locked.node,
        facts,
    };
    Ok(kind)
}

#[cfg(test)]
mod tests;
