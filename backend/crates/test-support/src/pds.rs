//! The throwaway PDS container: boot → readiness → endpoint → teardown-on-drop.

use std::time::Duration;

use anyhow::{Context as _, bail};
use rand::RngCore as _;
use serde_json::{Value, json};
use testcontainers::core::{Host, IntoContainerPort as _, Mount};
use testcontainers::runners::AsyncRunner as _;
use testcontainers::{ContainerAsync, GenericImage, ImageExt as _};

use crate::fixture::{ActingCredential, FixtureAccount};
use crate::plc_stub::StubPlc;

/// The port the reference-PDS image listens on (baked into the image as
/// `PDS_PORT=3000`).
const PDS_CONTAINER_PORT: u16 = 3000;

/// Docker's host alias the container uses to reach the stub PLC on the host
/// (mapped to the gateway via `host-gateway`; works on Docker 20.10+ and
/// Podman alike — verified on this repo's runtime and relied on in CI).
const PLC_HOST_ALIAS: &str = "host.docker.internal";

/// How long to wait for `/xrpc/_health` after the container starts (the node
/// process needs a few seconds; CI cold starts get generous headroom).
const HEALTH_TIMEOUT: Duration = Duration::from_secs(60);

/// A fresh, empty, hermetic reference PDS running in a container, plus the
/// in-process stub PLC it publishes identities to. Dropping the value tears
/// both down; nothing outlives the test.
pub struct ThrowawayPds {
    endpoint: String,
    client: reqwest::Client,
    plc: StubPlc,
    /// Held for its `Drop`: removing the guard removes the container.
    _container: ContainerAsync<GenericImage>,
}

impl ThrowawayPds {
    /// Boots the pinned reference-PDS image ([`crate::pds_image`]) with the
    /// hermetic environment, waits until `/xrpc/_health` answers, and returns
    /// the running instance.
    ///
    /// Mirrors the Postgres testcontainers pattern: the container handle
    /// lives inside the returned value, so it survives exactly as long as
    /// the test holds the `ThrowawayPds`.
    pub async fn boot() -> anyhow::Result<Self> {
        // Each instance gets its own PLC directory: per-instance state is
        // what makes two booted PDSes provably share nothing.
        let plc = StubPlc::spawn().await?;
        let plc_url = format!("http://{PLC_HOST_ALIAS}:{}", plc.port());

        // A structurally valid secp256k1 scalar (a raw random 32-byte string
        // is not guaranteed to be one; an invalid key crashes the PDS boot).
        let rotation_key = k256::SecretKey::random(&mut rand::rngs::OsRng);
        let rotation_key_hex = data_encoding::HEXLOWER.encode(&rotation_key.to_bytes());

        let image_ref = crate::pds_image();
        let (name, tag) = crate::split_image_ref(&image_ref);
        let mut request = GenericImage::new(name, tag)
            .with_exposed_port(PDS_CONTAINER_PORT.tcp())
            // The image ships no /pds; the PDS refuses to boot without its
            // data directory. tmpfs: RAM-backed, gone with the container.
            .with_mount(Mount::tmpfs_mount("/pds"))
            .with_host(PLC_HOST_ALIAS, Host::HostGateway)
            .with_startup_timeout(Duration::from_secs(120));
        for (key, value) in crate::hermetic_pds_env(
            &plc_url,
            &rotation_key_hex,
            &random_hex(32),
            &random_hex(16),
        ) {
            request = request.with_env_var(key, value);
        }

        let container = request
            .start()
            .await
            .context("start throwaway PDS container (is a container runtime socket available?)")?;
        let port = container
            .get_host_port_ipv4(PDS_CONTAINER_PORT)
            .await
            .context("mapped PDS port")?;
        let endpoint = format!("http://127.0.0.1:{port}");

        let client = reqwest::Client::new();
        wait_for_health(&client, &endpoint).await?;

        Ok(Self {
            endpoint,
            client,
            plc,
            _container: container,
        })
    }

    /// Base URL of this PDS on the host: `http://127.0.0.1:{mapped port}`.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Creates a fixture account (`com.atproto.server.createAccount`) on this
    /// PDS and returns the seam downstream tests act through.
    ///
    /// The handle must use the PDS's dev handle domain — `PDS_HOSTNAME=localhost`
    /// makes that `.test` (e.g. `alice.test`). Email and password are
    /// generated; the returned [`ActingCredential`] is the way to act.
    pub async fn provision_account(&self, handle: &str) -> anyhow::Result<FixtureAccount> {
        let response = self
            .client
            .post(format!(
                "{}/xrpc/com.atproto.server.createAccount",
                self.endpoint
            ))
            .json(&json!({
                "handle": handle,
                "email": format!("{}@example.com", handle.replace('.', "-")),
                "password": random_hex(16),
            }))
            .send()
            .await
            .context("createAccount request")?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .context("createAccount response body")?;
        if !status.is_success() {
            bail!("createAccount for {handle} failed ({status}): {body}");
        }
        let field = |name: &str| -> anyhow::Result<String> {
            Ok(body
                .get(name)
                .and_then(Value::as_str)
                .with_context(|| format!("createAccount response missing {name}: {body}"))?
                .to_string())
        };

        Ok(FixtureAccount {
            endpoint: self.endpoint.clone(),
            did: field("did")?,
            handle: field("handle")?,
            credential: ActingCredential::PdsSession {
                access_jwt: field("accessJwt")?,
                refresh_jwt: field("refreshJwt")?,
            },
        })
    }

    /// The DIDs this instance has published to its (local, per-instance) stub
    /// PLC — the hermeticity witness: identity minting landed here, not on
    /// any public directory.
    pub fn published_plc_dids(&self) -> Vec<String> {
        self.plc.recorded_dids()
    }
}

/// Polls `/xrpc/_health` until the PDS answers (readiness), bounded by
/// [`HEALTH_TIMEOUT`].
async fn wait_for_health(client: &reqwest::Client, endpoint: &str) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + HEALTH_TIMEOUT;
    loop {
        let probe = client
            .get(format!("{endpoint}/xrpc/_health"))
            .timeout(Duration::from_secs(2))
            .send()
            .await;
        if let Ok(response) = probe
            && response.status().is_success()
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "throwaway PDS at {endpoint} did not report healthy within \
                 {HEALTH_TIMEOUT:?} — inspect the container logs"
            );
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// `len` random bytes, hex-encoded — throwaway secrets for a throwaway PDS.
fn random_hex(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    data_encoding::HEXLOWER.encode(&bytes)
}
