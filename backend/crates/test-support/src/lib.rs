//! Shared test rig for the atproto (public-data) boundary — ZMVP-103.
//!
//! Boots a **throwaway PDS** in a container per test, provisions a fixture
//! account on it, and tears everything down on drop — the same isolation the
//! Postgres testcontainers harness already gives the private boundary,
//! extended to the atproto side. Scope is deliberately the PDS fixture seam
//! only: the existing inline-per-file Postgres pattern is not retrofitted.
//!
//! # Writing a PDS-backed integration test
//!
//! ```no_run
//! # async fn demo() -> anyhow::Result<()> {
//! use test_support::{ActingCredential, ThrowawayPds};
//!
//! let pds = ThrowawayPds::boot().await?;                    // fresh, empty, hermetic
//! let account = pds.provision_account("alice.test").await?; // the ZMVP-105 seam
//! let bearer = match &account.credential {
//!     ActingCredential::PdsSession { access_jwt, .. } => access_jwt.clone(),
//!     _ => unreachable!("new credential variants opt in explicitly"),
//! };
//! // ... act against `account.endpoint` as `account.did` ...
//! drop(pds);                                                // container + state gone
//! # Ok(())
//! # }
//! ```
//!
//! Requires a container runtime socket (`DOCKER_HOST` honored), exactly like
//! the Postgres-based suites, and a Tokio runtime (`#[tokio::test]`).
//!
//! # Hermeticity
//!
//! The rig makes **zero requests to the public atproto network**. Each
//! [`ThrowawayPds`] owns an in-process stub PLC directory on an ephemeral
//! loopback port; the container reaches it through the Docker `host-gateway`
//! alias, so identity minting (`did:plc` genesis operations) lands at the stub
//! and nowhere else — [`ThrowawayPds::published_plc_dids`] exposes what
//! arrived, letting tests assert the publication was local. No appview,
//! crawler, or report-service endpoint is ever configured.
//!
//! # Container reuse (escape hatch, off by default)
//!
//! One PDS boots per `ThrowawayPds::boot()`. If CI boot time ever hurts,
//! share one instance per test binary instead of changing CI infrastructure:
//! `ThrowawayPds` is `Send + Sync`, so a `tokio::sync::OnceCell<ThrowawayPds>`
//! in a test's common module (with per-test unique handles) is the intended
//! lever — the same reuse escape hatch the Postgres harness leaves available.

mod fixture;
mod pds;
mod plc_stub;

pub use fixture::{ActingCredential, FixtureAccount};
pub use pds::ThrowawayPds;

// The container-reuse escape hatch documented above rests on `ThrowawayPds`
// being shareable across tests; pin the auto-traits so a future field can't
// silently revoke the lever.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ThrowawayPds>();
};

/// The reference-PDS image the throwaway harness boots.
///
/// Must stay **the same literal** as the canonical `ZURFUR_PDS_IMAGE` pin in
/// `.env.example` (owned by ZMVP-102, the dev-loop lane — one image, two
/// lifecycles). The `default_image_matches_env_example` test asserts the two
/// never drift; update both together.
pub const DEFAULT_PDS_IMAGE: &str = "ghcr.io/bluesky-social/pds@sha256:1fa8bbceabb65d8e1710749b1ea92c1c20a7489ca38da4a0a5f64c0c10a70c29";

/// The image reference the harness will actually boot: the `ZURFUR_PDS_IMAGE`
/// environment variable when set (e.g. via `just test`'s dotenv), otherwise
/// [`DEFAULT_PDS_IMAGE`].
pub fn pds_image() -> String {
    image_ref_from(std::env::var("ZURFUR_PDS_IMAGE").ok())
}

/// Pure core of [`pds_image`]: pick the override when it is set and non-empty.
fn image_ref_from(override_var: Option<String>) -> String {
    match override_var {
        Some(image) if !image.trim().is_empty() => image,
        _ => DEFAULT_PDS_IMAGE.to_string(),
    }
}

/// Splits a Docker image reference into the `(name, tag)` pair
/// `testcontainers::GenericImage::new` expects, keeping any `@sha256:…`
/// digest attached to the tag (Docker accepts `name:tag@digest` pull refs;
/// verified against this machine's runtime).
fn split_image_ref(image: &str) -> (String, String) {
    let (name_tag, digest) = match image.split_once('@') {
        Some((name_tag, digest)) => (name_tag, Some(digest)),
        None => (image, None),
    };
    // The last ':' separates the tag — unless it belongs to a registry port
    // (i.e. a '/' follows it, as in `localhost:5000/pds`).
    //
    // A tagless digest pin deliberately gets the placeholder `latest`:
    // GenericImage formats the pull ref as `name:tag`, yielding
    // `name:latest@sha256:…`, where the digest overrides the tag — verified
    // against this repo's container runtime. Don't "simplify" the `latest@`.
    let (name, tag) = match name_tag.rfind(':') {
        Some(i) if !name_tag[i..].contains('/') => (&name_tag[..i], &name_tag[i + 1..]),
        _ => (name_tag, "latest"),
    };
    let tag = match digest {
        Some(digest) => format!("{tag}@{digest}"),
        None => tag.to_string(),
    };
    (name.to_string(), tag)
}

/// The full environment the throwaway PDS boots with. Pure so the hermeticity
/// tripwire test can inspect it: every endpoint the PDS is told about must be
/// one we control (the loopback stub PLC) — never a public atproto host.
///
/// Grounded in observed behavior of `ghcr.io/bluesky-social/pds:0.4`
/// (`@atproto/pds` 0.5.9), booted empirically during ZMVP-103:
/// - `PDS_DATA_DIRECTORY` must exist → the harness tmpfs-mounts `/pds`.
/// - `PDS_HOSTNAME=localhost` gives an `http://` public URL and `.test`
///   service-handle domains.
/// - Without `PDS_DEV_MODE=true` the OAuth provider rejects the non-HTTPS
///   public URL at startup ("Resource URL must use the https scheme").
/// - `PDS_DID_PLC_URL` left unset would default to the public
///   `https://plc.directory` — always pointed at the local stub instead.
/// - No appview / crawler / report-service variables: unset means the PDS
///   has no outbound dependency to reach.
fn hermetic_pds_env(
    plc_url: &str,
    rotation_key_hex: &str,
    jwt_secret: &str,
    admin_password: &str,
) -> Vec<(&'static str, String)> {
    vec![
        ("PDS_HOSTNAME", "localhost".to_string()),
        ("PDS_DID_PLC_URL", plc_url.to_string()),
        (
            "PDS_PLC_ROTATION_KEY_K256_PRIVATE_KEY_HEX",
            rotation_key_hex.to_string(),
        ),
        ("PDS_JWT_SECRET", jwt_secret.to_string()),
        ("PDS_ADMIN_PASSWORD", admin_password.to_string()),
        ("PDS_DATA_DIRECTORY", "/pds".to_string()),
        ("PDS_BLOBSTORE_DISK_LOCATION", "/pds/blocks".to_string()),
        ("PDS_INVITE_REQUIRED", "false".to_string()),
        ("PDS_DEV_MODE", "true".to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_override_wins_when_set() {
        assert_eq!(
            image_ref_from(Some("example.test/pds:9.9".into())),
            "example.test/pds:9.9"
        );
    }

    #[test]
    fn image_default_used_when_override_absent_or_empty() {
        assert_eq!(image_ref_from(None), DEFAULT_PDS_IMAGE);
        assert_eq!(image_ref_from(Some(String::new())), DEFAULT_PDS_IMAGE);
    }

    #[test]
    fn split_image_ref_handles_tag_and_digest() {
        // The canonical pin is tagless (`name@digest`); the runtime resolves
        // by digest, ignoring the placeholder `latest` tag (verified against
        // this repo's container runtime).
        let (name, tag) = split_image_ref(DEFAULT_PDS_IMAGE);
        assert_eq!(name, "ghcr.io/bluesky-social/pds");
        assert_eq!(
            tag,
            "latest@sha256:1fa8bbceabb65d8e1710749b1ea92c1c20a7489ca38da4a0a5f64c0c10a70c29"
        );

        assert_eq!(
            split_image_ref("pds:0.4@sha256:abc"),
            ("pds".into(), "0.4@sha256:abc".into())
        );
        assert_eq!(
            split_image_ref("postgres:16-alpine"),
            ("postgres".into(), "16-alpine".into())
        );
        // A registry port is not a tag separator.
        assert_eq!(
            split_image_ref("localhost:5000/pds"),
            ("localhost:5000/pds".into(), "latest".into())
        );
    }

    /// AC4 tripwire: the PDS must only ever be pointed at endpoints we
    /// control. If someone edits the boot env to reference a public atproto
    /// host, this fails without needing a container.
    #[test]
    fn pds_env_is_hermetic_by_construction() {
        let env = hermetic_pds_env("http://host.docker.internal:19999", "aa", "bb", "cc");

        let forbidden = ["plc.directory", "bsky.app", "bsky.network", "bsky.social"];
        for (key, value) in &env {
            for host in forbidden {
                assert!(
                    !value.contains(host),
                    "{key} references public atproto host {host}: {value}"
                );
            }
        }

        let plc = env
            .iter()
            .find(|(k, _)| *k == "PDS_DID_PLC_URL")
            .expect("PLC URL must be configured (unset defaults to the public plc.directory)");
        assert_eq!(plc.1, "http://host.docker.internal:19999");

        // No invite gate: the fixture provisioner must be able to createAccount.
        let invite = env.iter().find(|(k, _)| *k == "PDS_INVITE_REQUIRED");
        assert_eq!(invite.map(|(_, v)| v.as_str()), Some("false"));

        // Nothing may configure an appview / crawler / report-service reach-out.
        for (key, _) in &env {
            assert!(
                !key.contains("APP_VIEW") && !key.contains("CRAWLERS") && !key.contains("REPORT"),
                "{key} would give the throwaway PDS an outbound dependency"
            );
        }
    }

    /// Image-pin drift guard (uow 28ca4f decision): the canonical
    /// `ZURFUR_PDS_IMAGE` literal lives in `.env.example` (ZMVP-102's file);
    /// this crate duplicates it as `DEFAULT_PDS_IMAGE`. The two must be equal.
    ///
    /// Until ZMVP-102 lands the key, the guard reports itself unarmed and
    /// passes — `/close-gaps --post` reconciles the literals across the two
    /// branches; once the key exists on main this test enforces them forever.
    #[test]
    fn default_image_matches_env_example() {
        let env_example = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.env.example");
        let content = std::fs::read_to_string(env_example)
            .expect(".env.example exists at the workspace root");

        let pinned = content.lines().find_map(|line| {
            // Accept both live keys and the repo's commented-default style.
            let line = line.trim().trim_start_matches('#').trim_start();
            line.strip_prefix("ZURFUR_PDS_IMAGE=")
        });

        match pinned {
            Some(value) => assert_eq!(
                value.trim(),
                DEFAULT_PDS_IMAGE,
                "test-support's DEFAULT_PDS_IMAGE drifted from .env.example's \
                 ZURFUR_PDS_IMAGE — the dev loop and the test rig must boot \
                 the same image (update both together)"
            ),
            None => eprintln!(
                "default_image_matches_env_example: UNARMED — .env.example has \
                 no ZURFUR_PDS_IMAGE yet (ZMVP-102 not merged); this guard \
                 activates as soon as the key lands"
            ),
        }
    }
}
