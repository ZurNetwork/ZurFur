//! In-process stub PLC directory.
//!
//! The reference PDS refuses to mint `did:plc` accounts unless it can submit
//! the genesis operation to its configured PLC directory (verified against
//! `ghcr.io/bluesky-social/pds:0.4` / `@atproto/pds` 0.5.9: `createAccount`
//! with an unreachable `PDS_DID_PLC_URL` fails with `UpstreamFailure`,
//! "Unable to perform PLC operation"). Rather than reach the public
//! `plc.directory` — forbidden by the epic's hermeticity invariant — each
//! throwaway PDS gets its own loopback directory implementing the slice of
//! the PLC HTTP surface the PDS actually uses:
//!
//! - `POST /{did}` — record the signed genesis operation (observed at
//!   account creation).
//! - `GET /{did}` — the W3C DID document, derived from the recorded
//!   operation (observed from the PDS's identity resolver, e.g. during
//!   `com.atproto.repo.describeRepo`).
//! - `GET /{did}/data` — the operation's document data.
//!
//! Per-instance state doubles as the isolation and hermeticity witness: two
//! stubs share nothing, and every identity a PDS publishes is visible in
//! [`StubPlc::recorded_dids`] — proof the publication landed locally.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};

type OpStore = Arc<Mutex<HashMap<String, Value>>>;

/// A stub PLC directory on an ephemeral port, owned by one [`crate::ThrowawayPds`].
///
/// Binds `0.0.0.0` so the container can reach it through the Docker
/// `host-gateway` alias; the server task is aborted on drop.
pub(crate) struct StubPlc {
    port: u16,
    ops: OpStore,
    server: tokio::task::JoinHandle<()>,
}

impl StubPlc {
    /// Starts the stub on an OS-assigned port. Needs a running Tokio runtime.
    pub(crate) async fn spawn() -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", 0))
            .await
            .context("bind stub PLC listener")?;
        let port = listener.local_addr().context("stub PLC local addr")?.port();

        let ops: OpStore = Arc::new(Mutex::new(HashMap::new()));
        let app = Router::new()
            .route("/{did}", get(did_doc).post(record_op))
            .route("/{did}/data", get(did_data))
            .with_state(ops.clone());

        let server = tokio::spawn(async move {
            // Runs until the handle is aborted on drop; serve errors only if
            // the listener dies, which the owning test will surface anyway.
            let _ = axum::serve(listener, app).await;
        });

        Ok(Self { port, ops, server })
    }

    /// The port the container-side `PDS_DID_PLC_URL` must point at (via the
    /// `host-gateway` alias).
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Every DID whose genesis operation was submitted here — the hermeticity
    /// witness: identities the PDS published, provably to *this* stub.
    pub(crate) fn recorded_dids(&self) -> Vec<String> {
        self.ops
            .lock()
            .expect("stub PLC lock")
            .keys()
            .cloned()
            .collect()
    }
}

impl Drop for StubPlc {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// `POST /{did}` — store the (already signed) PLC operation as-is.
async fn record_op(
    Path(did): Path<String>,
    State(ops): State<OpStore>,
    axum::Json(op): axum::Json<Value>,
) -> Response {
    ops.lock().expect("stub PLC lock").insert(did, op);
    axum::Json(json!({})).into_response()
}

/// `GET /{did}` — the W3C DID document derived from the recorded operation,
/// in the same shape the real `plc.directory` serves (the PDS's resolver
/// rejects malformed documents).
async fn did_doc(Path(did): Path<String>, State(ops): State<OpStore>) -> Response {
    let Some(op) = ops.lock().expect("stub PLC lock").get(&did).cloned() else {
        return (StatusCode::NOT_FOUND, "DID not registered").into_response();
    };

    let verification_methods: Vec<Value> = as_object(op.get("verificationMethods"))
        .into_iter()
        .map(|(name, key)| {
            let multibase = key.as_str().unwrap_or_default();
            let multibase = multibase.strip_prefix("did:key:").unwrap_or(multibase);
            json!({
                "id": format!("{did}#{name}"),
                "type": "Multikey",
                "controller": did,
                "publicKeyMultibase": multibase,
            })
        })
        .collect();

    let services: Vec<Value> = as_object(op.get("services"))
        .into_iter()
        .map(|(name, svc)| {
            json!({
                "id": format!("#{name}"),
                "type": svc.get("type").cloned().unwrap_or(Value::Null),
                "serviceEndpoint": svc.get("endpoint").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();

    axum::Json(json!({
        "@context": [
            "https://www.w3.org/ns/did/v1",
            "https://w3id.org/security/multikey/v1",
            "https://w3id.org/security/suites/secp256k1-2019/v1",
        ],
        "id": did,
        "alsoKnownAs": op.get("alsoKnownAs").cloned().unwrap_or_else(|| json!([])),
        "verificationMethod": verification_methods,
        "service": services,
    }))
    .into_response()
}

/// `GET /{did}/data` — the operation's document data (op minus signature
/// envelope), mirroring the real directory.
async fn did_data(Path(did): Path<String>, State(ops): State<OpStore>) -> Response {
    let Some(op) = ops.lock().expect("stub PLC lock").get(&did).cloned() else {
        return (StatusCode::NOT_FOUND, "DID not registered").into_response();
    };
    let field = |name: &str| op.get(name).cloned().unwrap_or(Value::Null);
    axum::Json(json!({
        "did": did,
        "verificationMethods": field("verificationMethods"),
        "rotationKeys": field("rotationKeys"),
        "alsoKnownAs": field("alsoKnownAs"),
        "services": field("services"),
    }))
    .into_response()
}

fn as_object(value: Option<&Value>) -> Vec<(String, Value)> {
    value
        .and_then(Value::as_object)
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}
