//! Use cases about [`Commission`]s.
//!
//! [`sweep_deadlines`] is the one act with no actor — the system recording a
//! lapsed deadline. Its wall-clock timer and leader election live in `api`, so
//! the policy stays deterministic at an injected instant.

use crate::ports::WithPorts;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            Commission, CommissionId, CommissionRefusal, CommissionState, FileNameError, LateFacts,
            MarkupError, NewChangelogEntry, owes_late,
        },
        user::UserId,
    },
    ports::{
        Database, DidBelongsToAnotherActor, ElementNotFound, UnitOfWork, UnknownSurface, UnknownTab,
    },
};

use crate::{
    common_error::{CommonError, NotFoundEntity},
    den::{DenError, DenNameError, write::DenWriteError},
    transaction,
};
pub(crate) mod act;
pub mod archive;
pub mod changelog;
pub mod create;
pub mod deadline;
pub mod delete;
pub(crate) mod den;
pub mod files;
pub mod invitations;
pub mod markup;
pub mod maturity;
pub mod notes;
pub mod rename;
pub mod seats;
pub mod slots;
pub mod status;
pub mod unarchive;
mod values;
pub mod view;

pub use values::CommissionValues;

/// Commission use cases, with the ports already bound. A namespace, not a
/// mediator: one `impl Commissions<'_>` block per use-case file.
#[derive(Clone, Copy, WithPorts)]
pub struct Commissions<'a> {
    #[ports(is_root = true)]
    ports: &'a crate::Ports,
}

impl<'a> Commissions<'a> {
    /// Bind the namespace to resolved dependencies.
    pub fn new(ports: &'a crate::Ports) -> Self {
        Self { ports }
    }

    /// The bag this namespace was built over.
    pub fn ports(&self) -> &'a crate::Ports {
        self.ports
    }
}

impl<'a> TryFrom<&'a crate::Ports> for Commissions<'a> {
    type Error = crate::MissingPort;

    /// Cannot fail: [`Ports`](crate::Ports) always carries a file store.
    fn try_from(ports: &'a crate::Ports) -> Result<Self, Self::Error> {
        Ok(Self::new(ports))
    }
}

impl<'a> From<&'a crate::App> for Commissions<'a> {
    /// Binds the namespace to the app's ports.
    fn from(app: &'a crate::App) -> Self {
        Self::try_from(app.ports()).expect("composition root supplies the blob store")
    }
}

pub type CommissionResult<T> = Result<T, CommissionError>;

/// Why a commission use case could not answer. `Display` stays terse and never
/// interpolates the cause; the cause rides
/// [`source`](std::error::Error::source).
#[derive(Debug, thiserror::Error)]
pub enum CommissionError {
    #[error(transparent)]
    CommonError(#[from] CommonError),
    /// The commission store failed; the unit of work rolled back whole.
    #[error("The commission is already at the requested state")]
    CommissionAlreadyAtState,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    #[error("Not a member")]
    NotAMember,
    #[error("Invalid state requested")]
    InvalidStateRequested,
    /// The uploaded filename failed
    /// [`FileName`](domain::elements::commission::FileName)'s validation gate.
    #[error("Invalid file name")]
    InvalidFileName(#[source] FileNameError),
    /// The uploaded content exceeded the configured upload cap.
    #[error("File too large")]
    FileTooLarge,
    /// The uploaded content was zero bytes.
    #[error("File is empty")]
    FileEmpty,
    /// No such file entry on this commission.
    #[error("File not found")]
    FileNotFound,
    #[error("File blob missing")]
    FileBlobMissing,
    #[error("Seat not found")]
    SeatNotFound,
    /// The Seat named is already occupied, so it cannot be invited to.
    #[error("Seat already filled")]
    SeatFilled,
    /// The tab named is not one of this commission's tabs. Fabricated and
    /// belonging-to-another are deliberately indistinguishable.
    #[error("Tab not found")]
    TabNotFound,
    /// The `(tab, surface)` pair names no surface this commission declares.
    #[error("Unknown surface")]
    UnknownSurface,
    /// A deadline-axis act on a commission that carries no deadline.
    #[error("No deadline")]
    NoDeadline,
    /// The commission stands Late — the system's word, not a participant's.
    #[error("Commission is late")]
    CommissionLate,
    /// The annotation failed [`Markup`](domain::elements::commission::Markup)'s
    /// numeric gate.
    #[error("Invalid markup")]
    InvalidMarkup(#[source] MarkupError),
    #[error("Incorrect content")]
    IncorrectContent,
    /// A title, file name or slot title the Den's name rule refuses.
    #[error("Invalid name")]
    InvalidName(#[source] DenNameError),
}

impl From<CommissionRefusal> for CommissionError {
    /// A commission rule's refusal as a commission answer. A refused lifecycle move and a
    /// Disputed commission answer as an invalid state for now; nothing owed is the system's
    /// skip, which no user act reaches, so reaching one is a store failure.
    fn from(refusal: CommissionRefusal) -> Self {
        match refusal {
            CommissionRefusal::NoDeadline => Self::NoDeadline,
            CommissionRefusal::CommissionLate => Self::CommissionLate,
            CommissionRefusal::LifecycleMoveRefused | CommissionRefusal::CommissionDisputed => {
                Self::InvalidStateRequested
            }
            CommissionRefusal::NothingOwed => {
                CommonError::Infrastructure(anyhow::Error::new(refusal)).into()
            }
        }
    }
}

impl From<anyhow::Error> for CommissionError {
    fn from(err: anyhow::Error) -> Self {
        if err.downcast_ref::<DidBelongsToAnotherActor>().is_some() {
            return CommonError::DidBelongsToAnotherActor.into();
        } else if err.downcast_ref::<UnknownTab>().is_some() {
            return Self::TabNotFound;
        } else if err.downcast_ref::<UnknownSurface>().is_some() {
            return Self::UnknownSurface;
        } else if err.downcast_ref::<ElementNotFound>().is_some() {
            return CommonError::NotFound(NotFoundEntity::Element).into();
        }
        Self::CommonError(err.into())
    }
}
/// The closed door: resolve the commission for an actor who must be a
/// Participant of it. Anyone else is answered
/// [`NotAMember`](CommissionError::NotAMember), which the drivers render
/// byte-identically to an absent commission — never `403`.
pub(crate) async fn require_participant(
    ports: &crate::Ports,
    commission_id: &CommissionId,
    actor_id: &UserId,
) -> CommissionResult<Commission> {
    let commission = ports
        .commissions
        .find(commission_id)
        .await?
        .ok_or(CommonError::NotFound(NotFoundEntity::Commission))?;

    if !ports
        .commissions
        .is_participant(&commission.id, actor_id)
        .await?
    {
        return Err(CommissionError::NotAMember);
    }
    Ok(commission)
}

/// Resolve the commission for an act only its owner may perform. The owner
/// passes; a non-owner Participant gets
/// [`InsufficientPermissions`](CommissionError::InsufficientPermissions);
/// everyone else gets the same closed door as [`require_participant`].
pub(crate) async fn require_owner(
    ports: &crate::Ports,
    commission_id: &CommissionId,
    actor_id: &UserId,
) -> CommissionResult<Commission> {
    let commission = ports
        .commissions
        .find(commission_id)
        .await?
        .ok_or(CommonError::NotFound(NotFoundEntity::Commission))?;

    if commission.is_owned_by(actor_id) {
        return Ok(commission);
    }

    // Ownership before membership: the owner must never be locked out of their
    // own commission by a missing Participant row.
    if ports
        .commissions
        .is_participant(&commission.id, actor_id)
        .await?
    {
        return Err(CommissionError::InsufficientPermissions);
    }
    Err(CommissionError::NotAMember)
}

impl From<DenWriteError> for CommissionError {
    /// The Den's write refusal as a commission answer: its one not-found is the commission's,
    /// a role below the act's need is forbidden, and a refusal or a store failure is the
    /// store's, its cause kept on `source()`.
    fn from(refusal: DenWriteError) -> Self {
        match refusal {
            DenWriteError::NotFound => CommonError::commission_not_found().into(),
            DenWriteError::Forbidden => Self::InsufficientPermissions,
            failure @ (DenWriteError::Refused(_) | DenWriteError::Store(_)) => {
                CommonError::Infrastructure(anyhow::Error::new(failure)).into()
            }
        }
    }
}

/// Den's refusal as a commission answer: its one not-found becomes `not_found`, and a store
/// failure stays a store failure, never a miss. Not a `From`: the not-found depends on which
/// node was refused, and `From<anyhow::Error>`'s downcasts could turn a store failure into a 404.
pub(crate) fn den_refusal(refusal: DenError, not_found: CommissionError) -> CommissionError {
    match refusal {
        DenError::NotFound => not_found,
        DenError::Store(cause) => CommonError::Infrastructure(cause).into(),
    }
}

/// How many commissions one [`sweep_deadlines`] pass newly recorded as Late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepResult {
    pub marked_late: usize,
}

/// Run one deadline sweep as of the injected `now`: scan the lapsed deadlines
/// and append a system `Late` changelog entry for each one the Late rule says is
/// owed, the whole pass in one unit of work. A commission already Late is never
/// re-marked, and a Disputed one is skipped.
#[allow(
    clippy::disallowed_methods,
    reason = "the sweep is a system job, not a use-case method"
)]
pub async fn sweep_deadlines(
    database: &dyn Database,
    now: DateTimeUtc,
) -> Result<SweepResult, CommissionError> {
    let marked_late = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let lapsed = uow.commissions().lapsed_deadlines(now).await?;
        let mut marked = 0;
        for lapse in &lapsed {
            let commission = uow.commissions().find(&lapse.id).await?.ok_or_else(|| {
                anyhow::anyhow!("a lapsed commission is missing from the sweep's own unit")
            })?;
            // The scan answers only commissions with no `late` entry since their deadline
            // last changed, so each one it returns is owed one.
            let facts = LateFacts {
                state: CommissionState::from(&commission),
                owed: true,
            };
            // Log-only: `Late` is derived on lookup and never persisted.
            let Ok(mark) = owes_late(&facts, now) else {
                continue;
            };
            let entry = NewChangelogEntry::late(lapse.id, &mark, now);
            uow.changelog().append(&entry).await?;
            marked += 1;
        }
        Ok(marked)
    })
    .await
    .map_err(CommonError::Infrastructure)?;

    Ok(SweepResult { marked_late })
}

#[cfg(test)]
mod tests;
