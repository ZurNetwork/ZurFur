//! Commission file entries: intermediate work-in-progress a Participant uploads
//! into the review loop. Not a Product — private, Index-side, Total-tier content
//! that never crosses to a PDS.
//!
//! Two homes: the bytes live behind the [`FileStore`](crate::ports::FileStore)
//! port keyed by an opaque [`FileKey`], and the [`CommissionFile`] row records
//! that the entry belongs to a commission.

mod entity;
mod errors;
mod id;
mod rows;
mod value;

pub use entity::CommissionFile;
pub use errors::FileNameError;
pub use id::FileKey;
pub use rows::{FileDownload, FileMetadata};
pub use value::FileName;
