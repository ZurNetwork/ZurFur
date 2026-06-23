//! The one clock type the domain speaks in.
//!
//! Everything time-stamped in the domain (e.g. [`crate::elements::user::User`]'s
//! `created_at`, an [`crate::elements::account::Account`]'s `created_at` /
//! `updated_at` / `deleted_at`) is a [`DateTimeUtc`]. Times are *injected* into
//! constructors (`now: DateTimeUtc`), never read from a wall clock inside the
//! domain — that keeps the core pure and tests/import flows deterministic.

use chrono::{DateTime, Utc};

/// A UTC instant — the domain's single timestamp type.
///
/// An alias for [`chrono::DateTime<chrono::Utc>`](DateTime). Always UTC by
/// construction, so there is no zone to get wrong; convert to local time only at
/// the presentation edge, never in the domain.
pub type DateTimeUtc = DateTime<Utc>;
