//! Shared test rig for the atproto (public-data) boundary.
//!
//! Boots a throwaway PDS in a container per test, with an in-process stub
//! PLC directory so no request ever reaches the public atproto network,
//! provisions a fixture account, and tears everything down on drop. See
//! this directory's NODE.json for the usage recipe, the hermeticity
//! guarantees, and the container-reuse escape hatch.

pub mod contract;
mod fixture;
mod pds;
pub mod pg;
mod plc_stub;
pub mod runtime;

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
/// `.env.example`, which drives the real dev-loop container — one image
/// pinned in two places. The `default_image_matches_env_example` test
/// asserts the two never drift; update both together.
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
/// (`@atproto/pds` 0.5.9):
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
mod tests;
