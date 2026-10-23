//! The acts a viewer may take on one commission now, as the den's write would decide them:
//! each act's needed role, its rule and Atta's checks, run with nothing written.

use strum::VariantArray;

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, CommissionStanding, CommissionState, CommissionTitle, DirectionStatus,
            LifecycleStep,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{act::CommissionAct, deadline, lifecycle::move_to, rename, status::direction},
    den::{
        act::DenAct,
        gate::{Judgement, judge},
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
    /// `act`'s word and `judged`, the gate's own checks on it, unless Atta's checks refuse the
    /// act the den's builder makes of it.
    fn with_atta(&self, judged: Judgement, act: CommissionAct) -> Result<Judged, DenWriteError> {
        let word = ActWord(act.spec().word);
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

    /// The rename to `title`.
    fn rename(&self, title: CommissionTitle) -> Result<Judged, DenWriteError> {
        let act = rename::Act {
            commission: self.commission,
            title,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::Rename(act))
    }

    /// The lifecycle move to `to`.
    fn move_to(&self, to: LifecycleStep) -> Result<Judged, DenWriteError> {
        let act = move_to::Act {
            commission: self.commission,
            to,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::MoveLifecycle(act))
    }

    /// The direction status set to `to`.
    fn set_direction(&self, to: DirectionStatus) -> Result<Judged, DenWriteError> {
        let act = direction::set::Act {
            commission: self.commission,
            to,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::SetDirection(act))
    }

    /// The direction status clear.
    fn clear_direction(&self) -> Result<Judged, DenWriteError> {
        let act = direction::clear::Act {
            commission: self.commission,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::ClearDirection(act))
    }

    /// The deadline set to `deadline`.
    fn set_deadline(&self, deadline: DateTimeUtc) -> Result<Judged, DenWriteError> {
        let act = deadline::set::Act {
            commission: self.commission,
            deadline,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::SetDeadline(act))
    }

    /// The deadline clear.
    fn clear_deadline(&self) -> Result<Judged, DenWriteError> {
        let act = deadline::clear::Act {
            commission: self.commission,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::ClearDeadline(act))
    }

    /// The Delayed flag raised.
    fn set_delayed(&self) -> Result<Judged, DenWriteError> {
        let act = deadline::status::set::Act {
            commission: self.commission,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::SetDelayed(act))
    }

    /// The Delayed flag taken down.
    fn clear_delayed(&self) -> Result<Judged, DenWriteError> {
        let act = deadline::status::clear::Act {
            commission: self.commission,
        };
        let judged = judge(&act, self.standing, self.state, self.now);
        self.with_atta(judged, CommissionAct::ClearDelayed(act))
    }
}

/// The act's word when some value of it passes: every value `judged` names is one act's.
fn listed(judged: &[Judged]) -> Option<ActWord> {
    judged
        .iter()
        .find(|value| value.judgement.passes())
        .map(|value| value.word)
}

/// What `viewer`, standing on `commission` as `standing` says, may do to it at `now`, given its
/// stored `state` and its `title`, as the den's write would decide each act: the role its
/// capability needs, its rule on `state`, then Atta's checks on the act the den builds, with
/// nothing written. An act that takes a value is listed when some value passes: every lifecycle
/// step, every direction status, the rename to `title`, and a deadline at `now`, since the
/// deadline rule refuses by state, never by the time set. Someone who holds no role may do
/// nothing.
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
    let moves = LifecycleStep::VARIANTS
        .iter()
        .map(|step| probe.move_to(step.clone()))
        .collect::<Result<Vec<Judged>, DenWriteError>>()?;
    let lifecycle_targets = LifecycleStep::VARIANTS
        .iter()
        .zip(&moves)
        .filter(|(_, judged)| judged.judgement == Judgement::Changes)
        .map(|(step, _)| step.clone())
        .collect();
    let direction_sets = DirectionStatus::VARIANTS
        .iter()
        .map(|status| probe.set_direction(*status))
        .collect::<Result<Vec<Judged>, DenWriteError>>()?;

    let each_act = [
        vec![probe.rename(title.clone())?],
        moves,
        direction_sets,
        vec![probe.clear_direction()?],
        vec![probe.set_deadline(now)?],
        vec![probe.clear_deadline()?],
        vec![probe.set_delayed()?],
        vec![probe.clear_delayed()?],
    ];
    let acts = each_act
        .iter()
        .filter_map(|judged| listed(judged))
        .collect();
    let allowed = AllowedActs {
        acts,
        lifecycle_targets,
    };
    Ok(allowed)
}

#[cfg(test)]
mod tests;
