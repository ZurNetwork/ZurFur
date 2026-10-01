//! Workflows — an account's boards: ordered columns of commission cards.
//!
//! A Workflow knows about commissions, never the reverse. Columns and cards are
//! ordered by [`Position`], a base-62 fractional key compared bytewise, so an
//! insert mints a key between its neighbours and nothing is renumbered — a store
//! must order it bytewise too (`text COLLATE "C"`).

mod entity;
mod errors;
mod id;
mod ordering;
mod position;
mod value;

pub use entity::{Column, MAX_COLUMNS_PER_WORKFLOW, Workflow};
pub use errors::{PositionError, WorkflowError, WorkflowNameError};
pub use id::{ColumnId, WorkflowId};
pub use ordering::LexOrdering;
pub use position::Position;
pub use value::{ColumnName, WorkflowName};
