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

/// Submits a signed PLC operation for a DID. An adapter-local port (not a domain
/// port): the composition root selects an implementation via
/// [`plc_directory_from_config`], so `api` never names this trait. The operation is
/// passed as its already-serialized JSON body, so the same submitter handles a
/// genesis operation, a tombstone, or any later operation type without coupling to
/// their Rust shapes.
#[async_trait]
pub trait PlcDirectory: Send + Sync {
    /// Submit `operation` (its JSON body) registering/updating `did`. Fallible: the
    /// HTTP impl performs a network write; the no-op never fails.
    async fn submit(&self, did: &str, operation: &serde_json::Value) -> anyhow::Result<()>;
}

/// Local/dev directory: accepts the operation and does nothing. Used by the live
/// minter in ZMVP-49 so minting never touches the canonical `plc.directory`. Logs
/// only the DID (never key material or the operation body).
#[derive(Debug, Default, Clone)]
pub struct NoopPlcDirectory;

#[async_trait]
impl PlcDirectory for NoopPlcDirectory {
    async fn submit(&self, did: &str, _operation: &serde_json::Value) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plc::GenesisOperation;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn signed_op() -> crate::plc::SignedOperation {
        GenesisOperation::identity_only(
            vec!["did:key:a".to_string(), "did:key:b".to_string()],
            "did:key:c".to_string(),
            "alice.zurfur.app",
        )
        .into_signed("sig".to_string())
    }

    // The HTTP submitter must (1) trim a trailing slash on the base URL so the
    // target path is single-slashed `/{did}`, and (2) surface a non-2xx response as
    // an error. A one-shot local server captures the request line and replies 400.
    #[tokio::test]
    async fn http_directory_trims_trailing_slash_and_errors_on_non_2xx() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let request_line = Arc::new(Mutex::new(String::new()));
        let captured = request_line.clone();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            *captured.lock().unwrap() = req.lines().next().unwrap_or("").to_string();
            sock.write_all(
                b"HTTP/1.1 400 Bad Request\r\ncontent-length: 3\r\nconnection: close\r\n\r\nbad",
            )
            .await
            .unwrap();
            let _ = sock.shutdown().await;
        });

        // Base URL carries a trailing slash on purpose — it must be trimmed.
        let dir = HttpPlcDirectory::new(format!("http://{addr}/"));
        let res = dir
            .submit("did:plc:x", &signed_op().to_json().unwrap())
            .await;

        assert!(res.is_err(), "a non-2xx response must be an error");
        server.await.unwrap();
        let line = request_line.lock().unwrap().clone();
        assert!(
            line.starts_with("POST /did:plc:x "),
            "path must be single-slashed `/did:plc:x`, got request line: {line:?}"
        );
    }
}
