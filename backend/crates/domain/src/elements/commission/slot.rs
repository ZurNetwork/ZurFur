//! The commission's Slots: declared Character positions — a commission may
//! define them, title them, count them — whose filling is deferred to the
//! Character epic.
//!
//! Declaring one contributes an ordinary element typed
//! [`ElementType::slot`](super::ElementType::slot), with the title and notes in
//! a satellite row keyed by that element's id. Fill is unrepresentable: no shape
//! here carries an occupant, and an empty Slot is a valid permanent state.

mod entity;
mod errors;
mod rows;
mod values;

pub use entity::Slot;
pub use errors::SlotTitleError;
pub use rows::NewSlot;
pub use values::SlotTitle;
