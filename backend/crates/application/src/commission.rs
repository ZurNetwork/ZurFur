//! Use cases about [`Commission`](domain::elements::commission::Commission)s.
//!
//! The **deadline sweep** (ZMVP-86, conductor ruling E12) is the first of them —
//! and the one place the system acts on a commission rather than a Participant.
//! The deadline axis is otherwise entirely Participant-moved (the manual Delayed
//! flag, the deadline itself); the sweep's whole authority is: when a
//! commission's deadline has passed, say so in the changelog as a **system
//! entry** (no actor). It is provably scoped to exactly that — it calls
//! [`lapsed_deadlines`](domain::ports::CommissionWrites::lapsed_deadlines)
//! (which already excludes terminal lifecycles, already-Late commissions, and
//! anything without a deadline — AC4) and appends to the changelog; it holds no
//! handle that could move a Lifecycle or a direction status.
//!
//! [`sweep_deadlines`] is the whole policy. The wall-clock timer and the
//! advisory-lock leader election that drive it live in `api` — a driver
//! concern — so the policy stays deterministic and testable at an injected
//! instant.

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
/// mediator: every use-case file adds its own `impl Commissions<'_>` block
/// holding exactly one use case; helpers stay free functions.
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

    /// Cannot fail: [`Ports`](crate::Ports) carries a file store unconditionally,
    /// so [`MissingPort`](crate::MissingPort) is unreachable from here.
    fn try_from(ports: &'a crate::Ports) -> Result<Self, Self::Error> {
        Ok(Self::new(ports))
    }
}

impl<'a> From<&'a crate::App> for Commissions<'a> {
    /// Binds the namespace to the app's ports. The `expect` is unreachable while
    /// [`try_from`](Commissions::try_from) is infallible.
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

/// Why a commission use case could not answer. One enum per module: a driver
/// maps each variant to its own surface (problem+json, `{class, code}`).
///
/// `Display` is deliberately terse and never interpolates the cause — a store
/// error can carry SQL or constraint names, and a driver printing `{err}` must
/// not leak them. The cause stays on [`source`](std::error::Error::source) for
/// tracing.
#[derive(Debug)]
pub enum CommissionError {
    /// The commission store failed. The unit of work rolled back whole, so
    /// nothing was marked halfway; the caller may retry.
    Infrastructure(anyhow::Error),
    UserNotFound,
    CommissionNotFound,
    CommissionAlreadyAtState,
    InsufficientPermissions,
    NotAMember,
    InvalidStateRequested,
    /// The uploaded filename failed [`FileName`](domain::elements::commission::FileName)'s
    /// validation gate; the cause rides [`source`](std::error::Error::source).
    InvalidFileName(FileNameError),
    /// The uploaded content exceeded the caller's configured upload cap.
    FileTooLarge,
    /// The uploaded content was zero bytes.
    FileEmpty,
    /// No such file entry on this commission.
    FileNotFound,
    FileBlobMissing,
    SeatNotFound,
    /// The Seat named is already occupied, so it cannot be invited to — a state
    /// conflict, not a missing thing (ZMVP-78).
    SeatFilled,
    /// The tab named is not one of *this* commission's tabs — fabricated, or
    /// belonging to another commission. The two are deliberately
    /// indistinguishable.
    TabNotFound,
    /// The `(tab, surface)` pair names no surface this commission declares.
    UnknownSurface,
    /// The element named is not one of this commission's elements.
    ElementNotFound,
    /// A deadline-axis act on a commission that carries no deadline: there is
    /// nothing to be Delayed against.
    NoDeadline,
    /// The commission stands Late, and Late is the **system's** word — a
    /// participant cannot set or clear the axis over it.
    CommissionLate,
    /// The DID offered is already interned as a *different* kind of actor, so it
    /// cannot be provisioned as a User.
    DidBelongsToAnotherActor,
    /// The annotation failed [`Markup`](domain::elements::commission::Markup)'s
    /// numeric gate — a coordinate outside normalized 0–1 space, a degenerate
    /// extent, a blank or over-long comment. The cause rides
    /// [`source`](std::error::Error::source), mirroring
    /// [`InvalidFileName`](Self::InvalidFileName).
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

/// The **one** place a store error becomes a use-case error.
///
/// The stores raise a small set of typed errors through `anyhow` — the
/// composition-address gates ([`UnknownTab`], [`UnknownSurface`],
/// [`ElementNotFound`]) and the actor-kind conflict
/// ([`DidBelongsToAnotherActor`]) — each of which the wire already answers
/// precisely. Recognizing them here rather than at each call site is
/// deliberate: `?` is the only way a store error reaches a use case, so every
/// use case inherits the translation and none can quietly let a `409`/`404`
/// degrade into a `500`. Anything unrecognized stays
/// [`Infrastructure`](CommissionError::Infrastructure).
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

/// The **closed door**: resolve the commission for an actor who must be a
/// Participant of it, or refuse in a way that reveals nothing.
///
/// A non-participant is answered [`NotAMember`](CommissionError::NotAMember),
/// which the drivers render byte-identically to an absent commission's `404`.
/// Never [`InsufficientPermissions`](CommissionError::InsufficientPermissions):
/// a `403` confirms there is something here to be forbidden from, which is an
/// existence oracle over private work.
///
/// Lives here, once, because every act on a commission owes the same answer and
/// per-use-case copies drift (they already did — the api suite caught four).
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

/// The **managing-authority** gate: resolve the commission for an act only its
/// owner may perform.
///
/// Three answers, and the split matters. The owner passes. A Participant who is
/// *not* the owner already knows the commission exists, so they get an honest
/// [`InsufficientPermissions`](CommissionError::InsufficientPermissions) —
/// `403`. Everyone else gets the same closed door as
/// [`require_participant`]: `404`, indistinguishable from an absent commission.
///
/// This is the policy the driver-side `require_owner` held before the use cases
/// moved down (DD `55836674` D7); it is restated here because that is now where
/// authorization belongs.
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

    // Ownership is settled before membership is consulted, mirroring the gate
    // this replaces: the owner's Participant row is a permanent floor, so
    // asking is redundant for them — and were that row ever missing, the owner
    // must not be locked out of their own commission.
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

/// What one [`sweep_deadlines`] pass did, as the drivers render it: how many
/// commissions this pass newly recorded as Late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepResult {
    pub marked_late: usize,
}

/// Run **one** deadline sweep as of `now`.
///
/// A use case with no actor: `now` is injected (never read from a wall clock
/// here — the `datetime` doctrine, so the policy is deterministic by
/// construction) and the whole pass is **one unit of work** (ruling E12). The
/// candidate scan
/// ([`lapsed_deadlines`](domain::ports::CommissionWrites::lapsed_deadlines) —
/// deadline passed, not already Late, lifecycle not terminal) and each matching
/// **system** changelog entry (actor `NULL`, payload naming the missed
/// `deadline` and the standing flag — `delayed` or null — it replaced) commit
/// atomically or roll back together, so a swept commission without its Late
/// entry is unrepresentable (Changelog DD D4). A standing manual Delayed
/// upgrades to Late here (Engineer ruling 2026-07-05); a commission already
/// Late is never re-marked or re-logged — the *next* entry for the same
/// commission takes a fresh deadline miss (extend, then miss again).
///
/// A commission with no deadline never receives a deadline-axis value (AC4):
/// the scan cannot return one.
pub async fn sweep_deadlines(
    database: &dyn Database,
    now: DateTimeUtc,
) -> Result<SweepResult, CommissionError> {
    let marked_late = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let lapsed = uow.commissions().lapsed_deadlines(now).await?;
        for lapse in &lapsed {
            // Log-only: `Late` is derived on lookup and never persisted
            // (Engineer ruling 2026-07-08). This pass just records the
            // transition once, so hooks/plugins have an event to consume.
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
