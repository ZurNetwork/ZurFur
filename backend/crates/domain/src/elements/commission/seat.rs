//! The commission Seat: a 1:1 structural participant position — Creator,
//! Client, … — that exists before it is filled. A commission holds N Seats with
//! kinds repeating freely, and requirements ride on the vacant Seat.
//!
//! Seat is structural only: authority stays with Role, so [`SeatKind`] is an
//! open vocabulary. In the composition a Seat is an element typed
//! [`ElementType::seat`](super::ElementType::seat) with its interpreted data in
//! a satellite row keyed by the element's id.

mod entity;
mod errors;
mod rows;
mod values;

pub use entity::Seat;
pub use errors::{SeatKindError, SeatLinkError, SeatPromptError};
pub use rows::NewSeat;
pub use values::{SeatKind, SeatLink, SeatPrompt};
