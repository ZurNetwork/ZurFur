//! Global build-time configuration: numbers left to implementation, as
//! compile-time constants every layer reads the same way. Runtime
//! configuration (env, profiles, ports) is `composition::Config`; promoting a
//! knob here to an operator dial is future work.

use chrono::Duration;

/// The light anti-abuse ceiling on handle changes per account within
/// [`HANDLE_CHANGE_WINDOW`]: a burst throttle, not a long cooldown, since the
/// anti-impersonation weight lives on the quarantine rather than the cadence.
pub const HANDLE_CHANGE_LIMIT: i64 = 10;

/// The rolling window [`HANDLE_CHANGE_LIMIT`] is counted over.
pub const HANDLE_CHANGE_WINDOW: Duration = Duration::minutes(5);

/// How long a vacated `*.zurfur.app` handle stays reserved (quarantined) to the
/// account that left it before it frees for anyone else — the anti-impersonation
/// knob.
pub const HANDLE_QUARANTINE_WINDOW: Duration = Duration::days(30);
