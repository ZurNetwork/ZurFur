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

use crate::{ports::WithPorts, transaction};
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
#[derive(Debug)]
pub enum CommissionError {
    /// The commission store failed; the unit of work rolled back whole.
    Infrastructure(anyhow::Error),
    UserNotFound,
    CommissionNotFound,
    CommissionAlreadyAtState,
    InsufficientPermissions,
    NotAMember,
    InvalidStateRequested,
    /// The uploaded filename failed
    /// [`FileName`](domain::elements::commission::FileName)'s validation gate.
    InvalidFileName(FileNameError),
    /// The uploaded content exceeded the configured upload cap.
    FileTooLarge,
    /// The uploaded content was zero bytes.
    FileEmpty,
    /// No such file entry on this commission.
    FileNotFound,
    FileBlobMissing,
    SeatNotFound,
    /// The Seat named is already occupied, so it cannot be invited to.
    SeatFilled,
    /// The tab named is not one of this commission's tabs. Fabricated and
    /// belonging-to-another are deliberately indistinguishable.
    TabNotFound,
    /// The `(tab, surface)` pair names no surface this commission declares.
    UnknownSurface,
    /// The element named is not one of this commission's elements.
    ElementNotFound,
    /// A deadline-axis act on a commission that carries no deadline.
    NoDeadline,
    /// The commission stands Late — the system's word, not a participant's.
    CommissionLate,
    /// The DID offered is already interned as a different kind of actor.
    DidBelongsToAnotherActor,
    /// The annotation failed [`Markup`](domain::elements::commission::Markup)'s
    /// numeric gate.
    InvalidMarkup(MarkupError),
    IncorrectContent,
    AccountNotFound,
}

impl std::fmt::Display for CommissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Infrastructure(_) => write!(f, "the commission store failed"),
            Self::UserNotFound => write!(f, "The user could not be found"),
            Self::CommissionNotFound => write!(f, "The commission could not be found"),
            Self::CommissionAlreadyAtState => write!(f, "This commission is already in this state"),
            Self::InsufficientPermissions => write!(f, "Insufficient permissions to do this"),
            Self::NotAMember => write!(f, "Not a member of this commission"),
            Self::InvalidStateRequested => write!(f, "The state couldn't get set"),
            Self::InvalidFileName(_) => write!(f, "The filename is invalid"),
            Self::FileTooLarge => write!(f, "The file exceeds the upload limit"),
            Self::FileEmpty => write!(f, "The uploaded file is empty"),
            Self::FileNotFound => write!(f, "The file could not be found"),
            Self::FileBlobMissing => write!(f, "The blob seems to be missing"),
            Self::SeatNotFound => write!(f, "Seat not found"),
            Self::SeatFilled => write!(f, "That seat is already filled"),
            Self::TabNotFound => write!(f, "Tab not found"),
            Self::UnknownSurface => write!(f, "That surface is not declared here"),
            Self::ElementNotFound => write!(f, "Element not found"),
            Self::NoDeadline => write!(f, "This commission has no deadline"),
            Self::CommissionLate => write!(f, "Late is set by the system"),
            Self::DidBelongsToAnotherActor => write!(f, "That DID is already another actor"),
            Self::InvalidMarkup(_) => write!(f, "This markup is not valid"),
            Self::IncorrectContent => write!(f, "No content"),
            Self::AccountNotFound => write!(f, "Account not found"),
        }
    }
}

/// The one place a store error becomes a use-case error: the
/// composition-address gates ([`UnknownTab`], [`UnknownSurface`],
/// [`ElementNotFound`]) and [`DidBelongsToAnotherActor`] are recognized,
/// anything else stays [`Infrastructure`](CommissionError::Infrastructure).
impl From<anyhow::Error> for CommissionError {
    fn from(err: anyhow::Error) -> Self {
        if err.downcast_ref::<UnknownTab>().is_some() {
            Self::TabNotFound
        } else if err.downcast_ref::<UnknownSurface>().is_some() {
            Self::UnknownSurface
        } else if err.downcast_ref::<ElementNotFound>().is_some() {
            Self::ElementNotFound
        } else if err.downcast_ref::<DidBelongsToAnotherActor>().is_some() {
            Self::DidBelongsToAnotherActor
        } else {
            Self::Infrastructure(err)
        }
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
        .ok_or(CommissionError::CommissionNotFound)?;

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
        .ok_or(CommissionError::CommissionNotFound)?;

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

impl std::error::Error for CommissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Infrastructure(e) => Some(e.as_ref()),
            Self::InvalidFileName(e) => Some(e),
            Self::InvalidMarkup(e) => Some(e),
            _ => None,
        }
    }
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
    .map_err(CommissionError::Infrastructure)?;

    Ok(SweepResult { marked_late })
}
