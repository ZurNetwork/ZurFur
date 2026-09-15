//! The [`Commission`] — the platform's basic unit of work and the aggregator of
//! everything done under it.
//!
//! This module holds the envelope: id, title, owner, lifecycle, visibility,
//! deadline, maturity, statuses. A commission belongs to a User, never an
//! account, and survives account deletion. Its [`Visibility`] is the outermost
//! gate, applied before the composition's own [`effective_visibility`].

pub mod changelog;
pub mod element;
pub mod fact;
pub mod file;
pub mod markup;
pub mod seat;
pub mod seat_invitation;
pub mod slot;

mod deadline;
mod direction;
mod entity;
mod errors;
mod grant_level;
mod id;
mod lifecycle;
mod title;
mod visibility;

pub use changelog::{
    ChangelogEntry, ChangelogEntryKind, ChannelPointer, ChannelPointerError, NewChangelogEntry,
};
pub use deadline::{DeadlineStatus, LapsedDeadline, derive_deadline_status};
pub use direction::DirectionStatus;
pub use element::{
    Band, CommissionComposition, CompositionLabel, CompositionLabelError, DeclaredTab, ElementId,
    ElementPayload, ElementRow, ElementType, LABEL_MAX_CHARS, NewElement, SKELETON, SeatId,
    SurfaceAddress, SurfaceName, TabId, TabName, TabRow, VisibilityMode, declared_tabs,
    declares_surface,
};
pub use entity::Commission;
pub use errors::{
    CommissionTitleError, DeadlineStatusError, UnknownDirectionStatus, UnknownLifecycleStep,
    UnknownVisibility,
};
pub use fact::Fact;
pub use file::{CommissionFile, FileDownload, FileKey, FileMetadata, FileName, FileNameError};
pub use grant_level::GrantLevel;
pub use id::CommissionId;
pub use lifecycle::LifecycleStep;
pub use markup::{CommissionMarkup, Markup, MarkupError, MarkupKey, MarkupShape};
pub use seat::{
    NewSeat, Seat, SeatKind, SeatKindError, SeatLink, SeatLinkError, SeatPrompt, SeatPromptError,
};
pub use seat_invitation::{SeatInvitation, SeatInvitationId};
pub use slot::{NewSlot, Slot, SlotTitle, SlotTitleError};
pub use title::CommissionTitle;
pub use visibility::Visibility;
