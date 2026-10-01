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

/// Image-pin drift guard: the canonical `ZURFUR_PDS_IMAGE` literal lives
/// in `.env.example`; this crate duplicates it as `DEFAULT_PDS_IMAGE`.
/// The two must be equal.
///
/// Until `.env.example` defines that key, the guard reports itself
/// unarmed and passes; once the key exists on main this test enforces
/// the two match forever.
#[test]
fn default_image_matches_env_example() {
    let env_example = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.env.example");
    let content =
        std::fs::read_to_string(env_example).expect(".env.example exists at the workspace root");

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
