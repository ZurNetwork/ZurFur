//! The server's log subscriber: the operator's filter, with the DNS client's own
//! events dropped whatever that filter says.

use tracing_subscriber::{EnvFilter, fmt::MakeWriter, layer::SubscriberExt as _};

/// The subscriber `main` installs: `filter` (`RUST_LOG` or the config) over
/// formatted output to `writer`, with every DNS-client event dropped
/// ([`composition::logging`]), so no directive can turn them back on.
pub fn subscriber<W>(filter: EnvFilter, writer: W) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let formatted = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .finish();
    formatted.with(composition::logging::without_dns_client())
}

#[cfg(test)]
mod tests;
