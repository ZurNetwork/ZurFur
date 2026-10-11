//! The resolver's fixed limits. Constants, not configuration: an operator has
//! no business loosening them.

use std::time::Duration;

use crate::guarded_http::DNS_TIMEOUT;

/// One whole port call, every DNS lookup and fetch it makes included.
pub(super) const PORT_CALL_DEADLINE: Duration = Duration::from_secs(20);

/// Where every `did:plc` document is read from.
pub(super) const PLC_DIRECTORY: &str = "https://plc.directory";

/// The longest DID the resolver looks up.
pub(super) const DID_MAX_LEN: usize = 2048;

/// The longest handle whose `_atproto.<handle>.` name fits DNS's 255-octet
/// limit; a longer one is resolved over HTTPS only.
pub(super) const TXT_HANDLE_MAX_LEN: usize = 244;

/// The resolver's two deadlines, overridable only in tests.
#[derive(Clone, Copy, Debug)]
pub(super) struct Deadlines {
    /// One port call.
    pub(super) port_call: Duration,
    /// One TXT lookup, every retry and name server included.
    pub(super) dns: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            port_call: PORT_CALL_DEADLINE,
            dns: DNS_TIMEOUT,
        }
    }
}
