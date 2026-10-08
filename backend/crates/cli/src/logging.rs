//! The CLI's log subscriber: `RUST_LOG` (default `warn`) to stderr, with the DNS
//! client's own events dropped whatever that filter says.

use tracing_subscriber::{EnvFilter, fmt::MakeWriter, layer::SubscriberExt as _};

/// The subscriber [`init_tracing`](crate::init_tracing) installs: `filter` over
/// plain formatted output to `writer`, every DNS-client event dropped
/// ([`composition::logging`]); the operator commands build the same resolver.
pub(crate) fn subscriber<W>(filter: EnvFilter, writer: W) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let formatted = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .finish();
    formatted.with(composition::logging::without_dns_client())
}

#[cfg(test)]
mod tests;
