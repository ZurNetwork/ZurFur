//! Submitting a signed PLC operation to a directory (`POST {base_url}/{did}`).
//!
//! Submission is a public-boundary dual write — a separate retryable step,
//! never inside a private unit of work. The composition root picks
//! [`HttpPlcDirectory`] or [`NoopPlcDirectory`] from [`DirectoryConfig`].

use async_trait::async_trait;

/// Submits a signed PLC operation for a DID, as its already-serialized JSON
/// body — genesis, tombstone or any later operation type. An adapter-local
/// port; implementations are selected by [`plc_directory_from_config`].
#[async_trait]
pub trait PlcDirectory: Send + Sync {
    /// Submit `operation` registering or updating `did`.
    async fn submit(&self, did: &str, operation: &serde_json::Value) -> anyhow::Result<()>;
}

/// Local/dev directory: accepts the operation and does nothing, so minting
/// never touches the canonical `plc.directory`. Logs only the DID.
#[derive(Debug, Default, Clone)]
pub struct NoopPlcDirectory;

#[async_trait]
impl PlcDirectory for NoopPlcDirectory {
    async fn submit(&self, did: &str, _operation: &serde_json::Value) -> anyhow::Result<()> {
        tracing::info!(%did, "PLC directory submission skipped (no-op directory; ZMVP-49 C2)");
        Ok(())
    }
}

/// Real submitter: `POST {base_url}/{did}` with the signed operation as JSON.
/// Only reached when [`DirectoryConfig::enabled`] is set.
pub struct HttpPlcDirectory {
    /// The directory base URL, no trailing slash.
    base_url: String,
    client: reqwest::Client,
}

impl HttpPlcDirectory {
    /// Build a submitter targeting `base_url` (trailing slash trimmed).
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl PlcDirectory for HttpPlcDirectory {
    async fn submit(&self, did: &str, operation: &serde_json::Value) -> anyhow::Result<()> {
        let url = format!("{}/{}", self.base_url, did);
        let resp = self.client.post(&url).json(operation).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            // Body may carry a PLC validation error; it contains no secret.
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("PLC directory rejected {did}: {status} {text}");
        }
        tracing::info!(%did, "PLC directory submission accepted");
        Ok(())
    }
}

/// Composition-root config for directory submission; `enabled` gates real
/// registration against the canonical `plc.directory`.
#[derive(Debug, Clone)]
pub struct DirectoryConfig {
    /// The directory base URL used when `enabled`.
    pub endpoint: String,
    /// Whether to actually submit (`true`) or use the no-op directory (`false`).
    pub enabled: bool,
}

/// Select the directory implementation from config: `HttpPlcDirectory` when
/// enabled, otherwise `NoopPlcDirectory`.
pub fn plc_directory_from_config(config: &DirectoryConfig) -> Box<dyn PlcDirectory> {
    if config.enabled {
        Box::new(HttpPlcDirectory::new(config.endpoint.clone()))
    } else {
        Box::new(NoopPlcDirectory)
    }
}

#[cfg(test)]
mod tests;
