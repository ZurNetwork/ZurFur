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
mod tests {
    use super::*;
    use crate::plc::PlcOperation;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn signed_op() -> crate::plc::SignedOperation {
        PlcOperation::identity_only(
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
