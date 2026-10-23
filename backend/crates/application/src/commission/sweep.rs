//! The deadline sweep: the system's own act, marking each lapsed commission Late through the
//! Den's write, one unit per commission. The system acts here alone: only this file can make
//! the token its act carries.

use domain::{
    datetime::DateTimeUtc,
    elements::commission::{
        AsSystem, CommissionId, CommissionRefusal, LateFacts, LateKinds, LateMark, LifecycleStep,
        owes_late,
    },
};

use crate::{
    Ports,
    commission::{CommissionResult, Commissions, act::CommissionAct, den::CommissionPlace},
    den::{
        Den, DenType, Handler, Lent, Place,
        act::{ActShape, ActSpec, Anchor, DenAct, EntryRule, GatedEntry, Handled, HandlerError},
        needs,
        write::{self, Actor, DenWriteError},
    },
    use_case,
};

/// The system's leave to act: its one field is private to this file, so nothing else can make
/// one, and no other code can act as the system.
pub(crate) struct SweepToken {
    _sealed: (),
}

impl SweepToken {
    /// A token for the tests that drive the system's act below the sweep.
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self { _sealed: () }
    }
}

/// The system's Late mark: the commission it is anchored on.
/// The den writes the commission its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
}

impl Act {
    /// A change to the commission's fields; it needs the system, read from its capability's
    /// marker.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        ActSpec {
            word: "mark_late",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsSystem;

    type Checked = LateMark;

    const ENTRY: EntryRule<LateMark> = EntryRule::Always(GatedEntry::LATE);

    /// The Late rule on the facts read under the lock at `now`: the mark, or nothing owed.
    fn precondition(
        &self,
        facts: &LateFacts,
        now: DateTimeUtc,
    ) -> Result<Option<LateMark>, CommissionRefusal> {
        owes_late(facts, now).map(Some)
    }

    /// Writes no field: answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

/// How many commissions one sweep newly recorded as Late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepResult {
    pub marked_late: usize,
}

/// What one commission's Late mark came to, beside a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marked {
    /// Its `late` entry landed.
    Late,
    /// Nothing was owed under the lock, or the commission is gone.
    Skipped,
}

/// One commission's write as the sweep counts it: nothing owed and not found are skips.
fn marked<T>(written: Result<T, DenWriteError>) -> Result<Marked, DenWriteError> {
    match written {
        Ok(_) => Ok(Marked::Late),
        Err(
            DenWriteError::NotFound | DenWriteError::Precondition(CommissionRefusal::NothingOwed),
        ) => Ok(Marked::Skipped),
        Err(failure) => Err(failure),
    }
}

impl Commissions<'_> {
    /// One deadline sweep at `now`: a scan on the pool, then the system's Late mark through
    /// the Den's write for each lapsed commission, each in its own unit, re-read under its
    /// lock. A commission that fails is logged by its id and the sweep goes on.
    #[use_case]
    pub async fn sweep_deadlines(
        &self,
        #[ports] ports: &Ports,
        now: DateTimeUtc,
    ) -> CommissionResult<SweepResult> {
        let not_lapsing = LifecycleStep::not_lapsing();
        let late_kinds = LateKinds::default();
        let lapsed = ports
            .commissions
            .lapsed_deadlines(now, &not_lapsing, &late_kinds)
            .await?;
        let mut marked_late = 0;
        for commission in lapsed {
            let act = Act { commission };
            let command = write::Command {
                actor: Actor::System(SweepToken { _sealed: () }),
                act: DenAct::Commission(CommissionAct::MarkLate(act)),
            };
            let written = Den::from(ports).write(command, now).await;
            match marked(written) {
                Ok(Marked::Late) => marked_late += 1,
                Ok(Marked::Skipped) => {}
                Err(failure) => tracing::warn!(
                    commission = %commission,
                    error = %failure,
                    "a commission's Late mark failed; the sweep goes on"
                ),
            }
        }
        Ok(SweepResult { marked_late })
    }
}

#[cfg(test)]
mod tests;
