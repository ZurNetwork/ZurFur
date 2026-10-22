//! The [`Commission`] — the platform's basic unit of work and the aggregator of
//! everything done under it.
//!
//! This module holds the envelope: id, title, owner, lifecycle, visibility,
//! deadline, maturity, statuses. A commission belongs to a User, never an
//! account, and survives account deletion. Its own [`Level`](kernel::elements::level::Level)
//! is the outermost gate, applied before the composition's own [`effective_visibility`].

pub mod changelog;
pub mod element;
pub mod fact;
pub mod file;
pub mod markup;
pub mod seat;
pub mod seat_invitation;
pub mod slot;

mod commission_role;
mod deadline;
mod direction;
mod entity;
mod errors;
mod grant_level;
mod id;
mod lifecycle;
mod role_marker;
mod rows;
mod standing;
mod start;
mod state;
#[cfg(test)]
mod test_commission;
mod title;

pub use changelog::{
    ChangelogEntry, ChangelogEntryKind, ChannelPointer, ChannelPointerError, NewChangelogEntry,
    OpenChangelogEntry, OpenKind,
};
pub use commission_role::CommissionRole;
pub use deadline::{
    DeadlineChange, DeadlineCleared, DeadlineSet, DeadlineStatus, DelayedChange, DelayedCleared,
    DelayedSet, LapsedDeadline, LateMark, clear_deadline, clear_delayed, owes_late,
    schedule_deadline, set_delayed,
};
pub use direction::{
    DirectionChange, DirectionCleared, DirectionSet, DirectionStatus, clear_direction,
    set_direction,
};
pub use element::{
    Band, CommissionComposition, CompositionLabel, CompositionLabelError, DeclaredTab, ElementId,
    ElementPayload, ElementRow, ElementType, LABEL_MAX_CHARS, NewElement, SKELETON, SeatId,
    SurfaceAddress, SurfaceName, TabId, TabName, TabRow, VisibilityMode, declared_tabs,
    declares_surface,
};
pub use entity::Commission;
pub use errors::{
    CommissionRefusal, DeadlineStatusError, UnknownDirectionStatus, UnknownLifecycleStep,
};
pub use fact::Fact;
pub use file::{
    CommissionFile, FileDownload, FileKey, FileMetadata, FileName, FileNameError, FileSummary,
};
pub use grant_level::GrantLevel;
pub use id::CommissionId;
pub use lifecycle::{LifecycleMove, LifecycleStep, move_lifecycle};
pub use markup::{CommissionMarkup, Markup, MarkupError, MarkupKey, MarkupShape};
pub use role_marker::{AsOwner, AsParticipant};
pub use rows::{CommissionBirth, CommissionSummary, NewCommission};
pub use seat::{
    NewSeat, Seat, SeatKind, SeatKindError, SeatLink, SeatLinkError, SeatPrompt, SeatPromptError,
};
pub use seat_invitation::{SeatInvitation, SeatInvitationId};
pub use slot::{NewSlot, Slot, SlotTitle, SlotTitleError};
pub use standing::{CommissionStanding, LockedStanding};
pub use start::CommissionStart;
pub use state::{CommissionState, LateFacts};
pub use title::CommissionTitle;
