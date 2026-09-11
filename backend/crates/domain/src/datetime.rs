//! The one clock type the domain speaks in. Times are injected into
//! constructors (`now: DateTimeUtc`), never read from a wall clock inside the
//! domain.

use chrono::{DateTime, Utc};

/// A UTC instant — the domain's single timestamp type. Convert to local time
/// only at the presentation edge.
pub type DateTimeUtc = DateTime<Utc>;
