//! Submitting a signed PLC operation to a directory.
//!
//! Registering a `did:plc` means POSTing its signed genesis operation to a PLC
//! directory (`POST {base_url}/{did}`). This submission is a **public-boundary
//! dual write** — a separate, retryable step, never inside a private unit of work
//! (DESIGN/"Domains and Applications"; no cross-store transaction).
//!
//! For ZMVP-49 the live minter uses [`NoopPlcDirectory`]: it does **not** register
//! against the canonical `plc.directory`. The real HTTP submitter
//! ([`HttpPlcDirectory`]) is wired and ready but **gated off by config** until
//! launch (C2). Which one the minter holds is chosen in the composition root from
//! [`DirectoryConfig`].

use async_trait::async_trait;

use crate::plc::SignedOperation;

/// Submits a signed PLC operation for a DID. An adapter-local port (not a domain
/// port) because its argument — the protocol-shaped [`SignedOperation`] — is
/// quarantined to this crate; the composition root selects an implementation via
/// [`plc_directory_from_config`], so `api` never names this trait.
#[async_trait]
pub trait PlcDirectory: Send + Sync {
    /// Submit the `signed` operation registering `did`. Fallible: the HTTP impl
    /// performs a network write; the no-op never fails.
    async fn submit(&self, did: &str, signed: &SignedOperation) -> anyhow::Result<()>;
}

/// Local/dev directory: accepts the operation and does nothing. Used by the live
/// minter in ZMVP-49 so minting never touches the canonical `plc.directory`. Logs
/// only the DID (never key material or the operation body).
#[derive(Debug, Default, Clone)]
pub struct NoopPlcDirectory;

#[async_trait]
impl PlcDirectory for NoopPlcDirectory {
    async fn submit(&self, did: &str, _signed: &SignedOperation) -> anyhow::Result<()> {
        tracing::info!(%did, "PLC directory submission skipped (no-op directory; ZMVP-49 C2)");
        Ok(())
    }
}

/// Real submitter: `POST {base_url}/{did}` with the signed operation as JSON. Kept
/// off the live path by config until launch; exercised only when
/// [`DirectoryConfig::enabled`] is set.
pub struct HttpPlcDirectory {
    /// The directory base URL, e.g. `https://plc.directory` (canonical) or a local
    /// `@did-plc/server`. No trailing slash.
    base_url: String,
    /// Shared HTTP client (rustls), reused across submissions.
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
    async fn submit(&self, did: &str, signed: &SignedOperation) -> anyhow::Result<()> {
        let url = format!("{}/{}", self.base_url, did);
        let body = signed.to_json()?;
        let resp = self.client.post(&url).json(&body).send().await?;
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

/// Composition-root config for directory submission (figment-loaded in `api`).
///
/// `enabled` gates real registration. In ZMVP-49 it is **off**, so
/// [`plc_directory_from_config`] returns a [`NoopPlcDirectory`] and no operation
/// reaches the canonical `plc.directory`. Flipping it on at launch (with
/// `endpoint = "https://plc.directory"`) switches to [`HttpPlcDirectory`] with no
/// code change.
#[derive(Debug, Clone)]
pub struct DirectoryConfig {
    /// The directory base URL used when `enabled`.
    pub endpoint: String,
    /// Whether to actually submit (`true`) or use the no-op directory (`false`).
    pub enabled: bool,
}

/// Select the directory implementation from config: [`HttpPlcDirectory`] when
/// enabled, otherwise the [`NoopPlcDirectory`] (the ZMVP-49 default).
pub fn plc_directory_from_config(config: &DirectoryConfig) -> Box<dyn PlcDirectory> {
    if config.enabled {
        Box::new(HttpPlcDirectory::new(config.endpoint.clone()))
    } else {
        Box::new(NoopPlcDirectory)
    }
}
