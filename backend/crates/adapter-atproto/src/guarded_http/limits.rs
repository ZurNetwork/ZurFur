//! The guarded client's fixed limits. Constants, not configuration: an operator
//! has no business loosening them.

use std::time::Duration;

/// The whole fetch, from connecting to the last body byte.
pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// One DNS lookup, every retry and name server included.
pub(crate) const DNS_TIMEOUT: Duration = Duration::from_secs(5);

/// The most body bytes one fetch reads before refusing the response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::From)]
pub(crate) struct BodyCap(u64);

impl BodyCap {
    /// 512 KiB: every identity fetch.
    pub(crate) const DEFAULT: Self = Self(512 * 1024);

    /// Whether a body of `length` bytes goes past the cap.
    pub(crate) fn is_exceeded_by(self, length: u64) -> bool {
        length > self.0
    }
}

/// The two timeouts, overridable only in tests.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timeouts {
    pub(crate) fetch: Duration,
    pub(crate) dns: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            fetch: FETCH_TIMEOUT,
            dns: DNS_TIMEOUT,
        }
    }
}
