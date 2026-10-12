//! The log filter every driver installs: the DNS client's own events never
//! reach a log, whatever the operator's filter says.

use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;

/// The DNS client's crates. hickory logs the name it was asked about (a
/// handle's domain) at `error` when an answer is malformed.
const DNS_CLIENT_TARGETS: [&str; 3] = ["hickory_resolver", "hickory_net", "hickory_proto"];

/// The layer that drops every DNS-client event; a driver adds it on top of its
/// own subscriber so no directive can turn those events back on.
pub fn without_dns_client<S: Subscriber>() -> impl Layer<S> {
    filter_fn(|metadata| is_loggable_target(metadata.target()))
}

/// Whether an event from `target` may be logged: anything but the DNS client's.
pub fn is_loggable_target(target: &str) -> bool {
    !DNS_CLIENT_TARGETS.iter().any(|dns| {
        target
            .strip_prefix(dns)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    })
}

#[cfg(test)]
mod tests;
