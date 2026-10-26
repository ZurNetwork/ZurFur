//! The commission's Slots: declared Character positions — a commission may
//! define them, title them, count them — whose filling is deferred to the
//! Character epic.
//!
//! Declaring one makes its node under the commission's `slots` entry, named by its title, and
//! contributes an ordinary element typed [`ElementType::slot`](super::ElementType::slot), with
//! the notes in a satellite row keyed by that element's id. Fill is unrepresentable: no shape
//! here carries an occupant, and an empty Slot is a valid permanent state.

mod entity;
mod rows;
mod values;

pub use entity::Slot;
pub use rows::{NewSlot, SlotBirth, SlotDeclaration};
pub use values::SlotTitle;
