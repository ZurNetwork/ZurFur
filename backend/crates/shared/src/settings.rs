//! Global build-time configuration: the numbers the design decisions leave to
//! implementation, as compile-time constants every layer reads the same way.
//! Runtime configuration (env, profiles, ports) is `composition::Config`;
//! promoting a knob to an operator dial is ZMVP-208.

use chrono::Duration;

/// The light anti-abuse ceiling on handle changes per account within
/// [`HANDLE_CHANGE_WINDOW`] (DD "Account Handle Change Flow" `27852802` §3 — Bluesky's
/// ~10-per-5-minutes spirit: a burst throttle, **not** a long cooldown, since the
/// anti-impersonation weight lives on the quarantine, not the cadence). A build-time
/// number the DD leaves to implementation.
pub const HANDLE_CHANGE_LIMIT: i64 = 10;

/// The rolling window [`HANDLE_CHANGE_LIMIT`] is counted over (DD `27852802` §3).
pub const HANDLE_CHANGE_WINDOW: Duration = Duration::minutes(5);

/// How long a vacated `*.zurfur.app` handle stays reserved (quarantined) to the account
/// that left it before it frees for anyone else (DD `27852802` §4) — the
/// anti-impersonation knob, a build-time number the DD leaves to implementation.
pub const HANDLE_QUARANTINE_WINDOW: Duration = Duration::days(30);
