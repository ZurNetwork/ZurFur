//! The commission's flat composition: typed elements contributed into
//! code-declared surfaces, grouped by tabs, with no parent pointers.
//!
//! Structure is code ([`SKELETON`]), modes are data. Effective visibility is
//! `min(tab, surface, element)` — [`effective_visibility`]. The raw composition
//! and its payloads implement no `serde::Serialize`, so content can only leave
//! through a projection that clamped it server-side.

mod composition;
mod errors;
mod id;
mod label;
mod rows;
mod skeleton;
mod visibility;

pub use composition::CommissionComposition;
pub use errors::CompositionLabelError;
pub use id::{ElementId, SeatId, TabId};
pub use label::{Band, CompositionLabel, ElementType, LABEL_MAX_CHARS, SurfaceName, TabName};
pub use rows::{ElementPayload, ElementRow, NewElement, SurfaceAddress, TabRow};
pub use skeleton::{DeclaredTab, SKELETON, declared_tabs, declares_surface};
pub use visibility::VisibilityMode;
