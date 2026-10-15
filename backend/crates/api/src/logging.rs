//! The server's log subscriber: the operator's filter, the DNS client's own
//! events dropped whatever that filter says, and the environment's ceiling.
//! In dev every span prints its close with its timing; elsewhere nothing more
//! verbose than `info` is ever written, so debug detail stays a dev-only thing.

use composition::Environment;
use tracing_subscriber::{
    EnvFilter,
    filter::LevelFilter,
    fmt::{MakeWriter, format::FmtSpan},
    layer::SubscriberExt as _,
};

/// Where the server's log filter came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
pub enum FilterSource {
    /// The `RUST_LOG` variable, set and readable.
    #[strum(serialize = "RUST_LOG")]
    RustLog,
    /// The profile's `log_level`.
    #[strum(serialize = "the profile's log_level")]
    Profile,
}

/// The filter the server runs with, and where it came from.
#[derive(Debug)]
pub struct ChosenFilter {
    /// The filter.
    pub filter: EnvFilter,
    /// Where it came from.
    pub source: FilterSource,
}

impl ChosenFilter {
    /// `rust_log` when it is set and parses, else the profile's `log_level`. Inherent: a
    /// choice between two inputs, not a conversion of one.
    pub fn new(rust_log: Option<&str>, log_level: &str) -> Self {
        let from_rust_log = rust_log.and_then(|directives| EnvFilter::try_new(directives).ok());
        let source = if from_rust_log.is_some() {
            FilterSource::RustLog
        } else {
            FilterSource::Profile
        };
        let filter = from_rust_log.unwrap_or_else(|| EnvFilter::new(log_level));
        Self { filter, source }
    }
}

/// The subscriber `main` installs: `filter` over formatted output to `writer`, every
/// DNS-client event dropped ([`composition::logging`]). In [`Environment::DEV`] each span
/// prints its close with its timing; in every other environment no span event prints and
/// nothing more verbose than `info` passes, whatever `filter` says.
pub fn subscriber<W>(
    env: &Environment,
    filter: EnvFilter,
    writer: W,
) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let (span_events, ceiling) = match env {
        Environment::DEV => (FmtSpan::CLOSE, LevelFilter::TRACE),
        Environment::STG | Environment::PROD => (FmtSpan::NONE, LevelFilter::INFO),
    };
    let formatted = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(span_events)
        .with_writer(writer)
        .finish();
    formatted
        .with(composition::logging::without_dns_client())
        .with(ceiling)
}

#[cfg(test)]
mod tests;
