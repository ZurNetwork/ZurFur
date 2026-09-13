//! Use cases about [`Commission`]s.
//!
//! [`sweep_deadlines`] is the one act with no actor — the system recording a
//! lapsed deadline. Its wall-clock timer and leader election live in `api`, so
//! the policy stays deterministic at an injected instant.

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, FileNameError, MarkupError,
            NewChangelogEntry,
        },
        user::UserId,
    },
    ports::{
        AccountStore, ChangelogStore, CommissionStore, Database, DidBelongsToAnotherActor,
        DidMinter, ElementNotFound, FileStore, UnitOfWork, UnknownSurface, UnknownTab, UserStore,
    },
};
use serde_json::json;

use crate::{
    common_error::{CommonError, NotFoundEntity},
    ports::WithPorts,
    transaction,
};
pub mod archive;
pub mod changelog;
pub mod create;
pub mod deadline;
pub mod delete;
pub mod files;
pub mod invitations;
pub mod list;
pub mod markup;
pub mod maturity;
pub mod notes;
pub mod seats;
pub mod slots;
pub mod status;
pub mod unarchive;
pub mod view;

/// Commission use cases, with the ports already bound. A namespace, not a
/// mediator: one `impl Commissions<'_>` block per use-case file.
#[derive(Clone, Copy)]
pub struct Commissions<'a> {
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

impl<'a> WithPorts<'a> for Commissions<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.ports
    }
}

pub struct CommissionPorts<'a> {
    pub commissions: &'a dyn CommissionStore,
    pub changelog: &'a dyn ChangelogStore,
    pub users: &'a dyn UserStore,
    pub accounts: &'a dyn AccountStore,
    pub did_minter: &'a dyn DidMinter,
    pub database: &'a dyn Database,
    pub files: &'a dyn FileStore,
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
    InvalidFileName(FileNameError),
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
    InvalidMarkup(MarkupError),
    #[error("Incorrect content")]
    IncorrectContent,
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

/// How many commissions one [`sweep_deadlines`] pass newly recorded as Late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepResult {
    pub marked_late: usize,
}

/// Run one deadline sweep as of the injected `now`: scan the lapsed deadlines
/// and append a system `Late` changelog entry for each, the whole pass in one
/// unit of work. A commission already Late is never re-marked; one without a
/// deadline is never returned by the scan.
pub async fn sweep_deadlines(
    database: &dyn Database,
    now: DateTimeUtc,
) -> Result<SweepResult, CommissionError> {
    let marked_late = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let lapsed = uow.commissions().lapsed_deadlines(now).await?;
        for lapse in &lapsed {
            // Log-only: `Late` is derived on lookup and never persisted.
            let entry = NewChangelogEntry::system(
                lapse.id,
                ChangelogEntryKind::Late,
                json!({
                    "deadline": lapse.deadline,
                    "from": lapse.status.map(|s| s.as_str()),
                }),
                now,
            );
            uow.changelog().append(&entry).await?;
        }
        Ok(lapsed.len())
    })
    .await
    .map_err(CommonError::Infrastructure)?;

    Ok(SweepResult { marked_late })
}
