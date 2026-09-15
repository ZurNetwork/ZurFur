//! The commission changelog: an append-only, immutable, per-commission record of
//! every domain event, and the platform's structured communication channel.
//! It is the only durable record of what happened — state is read directly
//! from its own tables, never replayed from this stream.
//!
//! Not a chat: free text enters only as note entries, standalone or attached,
//! and an entry cannot reference another — replies are unrepresentable.
//! Conversation lives in the external [`ChannelPointer`].

mod errors;
mod kind;
mod rows;
mod value;

pub use errors::ChannelPointerError;
pub use kind::ChangelogEntryKind;
pub use rows::{ChangelogEntry, NewChangelogEntry};
pub use value::ChannelPointer;
