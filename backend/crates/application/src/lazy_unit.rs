//! A unit of work opened on demand, and the nested-unit guard that goes with
//! the `#[use_case]` unit brackets.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use domain::ports::{Database, Unit, UnitOfWork};

const SPLIT_UNIT_MESSAGE: &str =
    "a use case opened its own unit after a use case it called opened and settled another";
const NESTED_UNIT_MESSAGE: &str = "a use case opened a unit inside another use case's unit";

/// What one unit-holding use case has done on this task, linked to the
/// use case that called it.
struct UnitFrame {
    opened: AtomicBool,
    inner_opened: AtomicBool,
    parent: Option<Arc<UnitFrame>>,
}

impl UnitFrame {
    /// Whether this frame or any enclosing one holds an open unit.
    fn chain_opened(&self) -> bool {
        self.opened.load(Ordering::Relaxed)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.chain_opened())
    }

    /// Records, on every enclosing frame, that a use case below it opened a unit.
    fn mark_enclosing_inner_opened(&self) {
        let mut ancestor = self.parent.as_ref();
        while let Some(frame) = ancestor {
            frame.inner_opened.store(true, Ordering::Relaxed);
            ancestor = frame.parent.as_ref();
        }
    }
}

tokio::task_local! {
    static UNIT_FRAME: Arc<UnitFrame>;
}

/// An unopened unit of work: the `#[lazy_unit]` injection. The body calls
/// [`open`](LazyUnit::open) right before its first write, so reads and external
/// calls before it hold no pool connection for a unit. The use-case wrapper
/// commits on `Ok` and rolls back on `Err` only if the unit was opened.
pub struct LazyUnit<'a> {
    database: &'a dyn Database,
    unit: Option<Box<dyn UnitOfWork>>,
}

impl<'a> LazyUnit<'a> {
    /// An unopened unit over `database`.
    pub fn new(database: &'a dyn Database) -> Self {
        Self {
            database,
            unit: None,
        }
    }

    /// Begins the unit on the first call; later calls return the same unit.
    #[allow(
        clippy::disallowed_methods,
        reason = "a LazyUnit is a sanctioned door: it begins the unit its use-case wrapper settles"
    )]
    pub async fn open(&mut self) -> anyhow::Result<Unit<'_>> {
        let unit = match self.unit.take() {
            Some(unit) => unit,
            None => {
                mark_unit_opened();
                self.database.begin().await?
            }
        };
        Ok(&mut **self.unit.insert(unit))
    }

    /// Ends the unit if it was opened: commit on `Ok`, rollback on `Err`. A
    /// unit that was never opened has nothing to settle.
    #[allow(
        clippy::disallowed_methods,
        reason = "a LazyUnit is a sanctioned door: it commits or rolls back the unit it opened"
    )]
    pub(crate) async fn settle<T, E>(self, outcome: Result<T, E>) -> Result<T, E>
    where
        E: From<anyhow::Error>,
    {
        let Some(unit) = self.unit else {
            return outcome;
        };
        match outcome {
            Ok(value) => {
                unit.commit().await?;
                Ok(value)
            }
            Err(error) => {
                // The use case's error is the meaningful one; a rollback
                // failure must never replace it.
                let _ = unit.rollback().await;
                Err(error)
            }
        }
    }
}

/// Runs a unit-holding use case's body with its frame on the task. `opened`
/// is true for an eager `#[unit]`, whose unit is already begun.
pub(crate) async fn within_unit<F: Future>(opened: bool, body: F) -> F::Output {
    let parent = UNIT_FRAME.try_with(Arc::clone).ok();
    let frame = Arc::new(UnitFrame {
        opened: AtomicBool::new(opened),
        inner_opened: AtomicBool::new(false),
        parent,
    });
    if opened {
        frame.mark_enclosing_inner_opened();
    }
    UNIT_FRAME.scope(frame, body).await
}

/// Debug-asserts that no enclosing use case holds a unit; called before an
/// eager `#[unit]` begins.
pub(crate) fn assert_no_unit_open() {
    let open = UNIT_FRAME
        .try_with(|frame| frame.chain_opened())
        .unwrap_or(false);
    debug_assert!(!open, "{NESTED_UNIT_MESSAGE}");
}

/// Records that this use case's unit is now open, debug-asserting that no
/// enclosing use case holds one and that no use case it called already did.
fn mark_unit_opened() {
    let _ = UNIT_FRAME.try_with(|frame| {
        let enclosing_opened = frame
            .parent
            .as_ref()
            .is_some_and(|parent| parent.chain_opened());
        debug_assert!(!enclosing_opened, "{NESTED_UNIT_MESSAGE}");
        debug_assert!(
            !frame.inner_opened.load(Ordering::Relaxed),
            "{SPLIT_UNIT_MESSAGE}"
        );
        frame.opened.store(true, Ordering::Relaxed);
        frame.mark_enclosing_inner_opened();
    });
}

#[cfg(test)]
mod tests;
