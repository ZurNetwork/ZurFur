//! The acts a viewer may take on one commission now, as the den's write would decide them:
//! each act's needed role, its rule and Atta's checks, run with nothing written.

use strum::VariantArray;

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionDeadline, CommissionId, CommissionStanding, CommissionState, CommissionTitle,
            DirectionStatus, LifecycleStep, NewSlot, SKELETON, SlotDeclaration, SlotTitle,
            SurfaceAddress, SurfaceName, TabId,
        },
        name::NodeName,
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        act::{CommissionAct, CommissionActKind},
        deadline,
        lifecycle::move_to,
        rename,
        slots::declare,
        status::direction,
    },
    den::{
        act::DenAct,
        gate::{Judgement, judge_act},
        write::{DenWriteError, atta_passes},
    },
};

/// An act's word, as the act's own spec names it: `rename`, `move_lifecycle`, and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct ActWord(&'static str);

/// What a viewer may do to one commission now: the words of the acts the den's write would
/// pass, in a fixed order, and the steps a lifecycle move would really move it to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AllowedActs {
    /// The acts' words.
    pub(crate) acts: Vec<ActWord>,
    /// The steps a move would change it to; never the step it holds.
    pub(crate) lifecycle_targets: Vec<LifecycleStep>,
}

/// One act the den's write could carry: its word, and what the gate's checks answer for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Judged {
    word: ActWord,
    judgement: Judgement,
}

/// One commission as a viewer stands on it, and what the den's write would read of it.
struct Probe<'a> {
    ports: &'a Ports,
    viewer: &'a UserId,
    commission: CommissionId,
    standing: CommissionStanding,
    state: &'a CommissionState,
    now: DateTimeUtc,
}

impl Probe<'_> {
    /// `act`'s word and what the gate's own checks answer for it, unless Atta's checks refuse
    /// the act the den's builder makes of it.
    fn probe(&self, act: CommissionAct) -> Result<Judged, DenWriteError> {
        let word = ActWord(act.spec().word);
        let judged = judge_act(&act, self.standing, self.state, self.now);
        let judgement = if judged.passes() {
            let level = self.standing.level;
            let body = DenAct::Commission(act);
            let atta = atta_passes(self.ports, self.viewer, self.commission, level, body)?;
            if atta { judged } else { Judgement::Refused }
        } else {
            Judgement::Refused
        };
        Ok(Judged { word, judgement })
    }
}

/// The values the list tries of the act of `kind` on `commission`, at `now` and given its
/// `title`: every lifecycle step, every direction status, the rename to `title`, a deadline at
/// `now`, since the deadline rule refuses by state, never by the time set, each clear and
/// Delayed act once, and a declare of one Slot titled `title` on the skeleton's first surface,
/// since no rule of the commission's state refuses one and its tab is read only by its write.
/// None of a create or of the system's Late mark, which no user takes on a stored commission.
/// A match over every kind, so a new act fails to compile until it is tried.
fn tried(
    kind: CommissionActKind,
    commission: CommissionId,
    title: &CommissionTitle,
    now: DateTimeUtc,
) -> Vec<CommissionAct> {
    match kind {
        CommissionActKind::Create | CommissionActKind::MarkLate => Vec::new(),
        CommissionActKind::Rename => {
            let act = rename::Act {
                commission,
                title: title.clone(),
            };
            vec![CommissionAct::Rename(act)]
        }
        CommissionActKind::MoveLifecycle => LifecycleStep::VARIANTS
            .iter()
            .map(|step| {
                let act = move_to::Act {
                    commission,
                    to: step.clone(),
                };
                CommissionAct::MoveLifecycle(act)
            })
            .collect(),
        CommissionActKind::SetDirection => DirectionStatus::VARIANTS
            .iter()
            .map(|status| {
                let act = direction::set::Act {
                    commission,
                    to: *status,
                };
                CommissionAct::SetDirection(act)
            })
            .collect(),
        CommissionActKind::ClearDirection => {
            let act = direction::clear::Act { commission };
            vec![CommissionAct::ClearDirection(act)]
        }
        CommissionActKind::SetDeadline => {
            let act = deadline::set::Act {
                commission,
                deadline: CommissionDeadline::from(now),
            };
            vec![CommissionAct::SetDeadline(act)]
        }
        CommissionActKind::ClearDeadline => {
            let act = deadline::clear::Act { commission };
            vec![CommissionAct::ClearDeadline(act)]
        }
        CommissionActKind::SetDelayed => {
            let act = deadline::status::set::Act { commission };
            vec![CommissionAct::SetDelayed(act)]
        }
        CommissionActKind::ClearDelayed => {
            let act = deadline::status::clear::Act { commission };
            vec![CommissionAct::ClearDelayed(act)]
        }
        CommissionActKind::DeclareSlot => first_surface()
            .map(|surface| {
                let slot = NewSlot {
                    address: SurfaceAddress::new(TabId::mint(), surface),
                    notes: None,
                };
                let act = declare::Act {
                    commission,
                    title: SlotTitle::from(NodeName::from(title.clone())),
                    declaration: SlotDeclaration::minted(slot),
                };
                CommissionAct::DeclareSlot(act)
            })
            .into_iter()
            .collect(),
    }
}

/// The skeleton's first declared surface, as a surface name.
fn first_surface() -> Option<SurfaceName> {
    SKELETON
        .iter()
        .flat_map(|declared| declared.surfaces)
        .find_map(|surface| surface.parse::<SurfaceName>().ok())
}

/// The step a lifecycle move moves to; `None` for every other act.
fn move_target(act: &CommissionAct) -> Option<LifecycleStep> {
    let CommissionAct::MoveLifecycle(move_to) = act else {
        return None;
    };
    Some(move_to.to.clone())
}

/// What `viewer`, standing on `commission` as `standing` says, may do to it at `now`, given its
/// stored `state` and its `title`, as the den's write would decide each act: the role its
/// capability needs, its rule on `state`, then Atta's checks on the act the den builds, with
/// nothing written. Each act is tried with the values [`tried`] names, in its kind's order, and
/// listed when some value passes; a step is a lifecycle target when its move changes the
/// commission. Someone who holds no role may do nothing.
pub(crate) fn allowed_acts(
    ports: &Ports,
    viewer: &UserId,
    commission: CommissionId,
    standing: CommissionStanding,
    state: &CommissionState,
    title: &CommissionTitle,
    now: DateTimeUtc,
) -> Result<AllowedActs, DenWriteError> {
    let probe = Probe {
        ports,
        viewer,
        commission,
        standing,
        state,
        now,
    };
    let mut acts = Vec::new();
    let mut lifecycle_targets = Vec::new();
    for kind in CommissionActKind::VARIANTS {
        let mut listed = None;
        for act in tried(*kind, commission, title, now) {
            let target = move_target(&act);
            let judged = probe.probe(act)?;
            if judged.judgement.passes() {
                listed.get_or_insert(judged.word);
            }
            if let (Some(step), Judgement::Changes) = (target, judged.judgement) {
                lifecycle_targets.push(step);
            }
        }
        acts.extend(listed);
    }
    let allowed = AllowedActs {
        acts,
        lifecycle_targets,
    };
    Ok(allowed)
}

#[cfg(test)]
mod tests;
