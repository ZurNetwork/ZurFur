//! The log filter every driver installs: the DNS client's own events never
//! reach a log, whatever the operator's filter says.

use tracing::Metadata;
use tracing_subscriber::filter::{FilterFn, filter_fn};

/// The DNS client's crates. hickory logs the name it was asked about (a
/// handle's domain) at `error` when an answer is malformed.
const DNS_CLIENT_TARGETS: [&str; 3] = ["hickory_resolver", "hickory_net", "hickory_proto"];

/// The filter layer that drops every DNS-client event; a driver adds it on top
/// of its own subscriber so no directive can turn those events back on.
pub type WithoutDnsClient = FilterFn<fn(&Metadata<'_>) -> bool>;

/// The layer that keeps only [`is_loggable_target`] events.
pub fn without_dns_client() -> WithoutDnsClient {
    filter_fn(is_loggable as fn(&Metadata<'_>) -> bool)
}

/// Whether an event from `target` may be logged: anything but the DNS client's.
pub fn is_loggable_target(target: &str) -> bool {
    !DNS_CLIENT_TARGETS.iter().any(|dns| {
        target
            .strip_prefix(dns)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    })
}

fn is_loggable(metadata: &Metadata<'_>) -> bool {
    is_loggable_target(metadata.target())
}

#[cfg(test)]
mod tests;
