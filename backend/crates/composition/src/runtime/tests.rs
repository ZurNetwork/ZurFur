use domain::elements::custody::{CustodyError, MacMessage, MacUse};
use zeroize::Zeroizing;

use crate::Environment;

use super::*;

/// A pool that never connects: wiring builds every adapter without a database.
const UNREACHABLE_DATABASE_URL: &str = "postgres://runtime-tests.invalid/none";

/// A dev configuration on loopback.
fn dev_config() -> Config {
    Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().expect("a loopback socket address"),
        public_url: "http://127.0.0.1:8080".parse().expect("a bare origin"),
        database_url: UNREACHABLE_DATABASE_URL.to_owned(),
        log_level: "info".to_owned(),
        handle_domain: "zurfur.app".parse().expect("a valid handle domain"),
        plc_directory_endpoint: "http://localhost:2582".to_owned(),
        plc_directory_submit: false,
        deadline_sweep_interval_secs: 300,
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        build_id: "unknown".try_into().expect("a build id"),
    }
}

/// A runtime wired over a lazy pool, holding the keys of `purposes` from a dev root.
fn wired(purposes: &[Purpose]) -> Runtime {
    let config = dev_config();
    let root = RootSecret::from(Zeroizing::new([0xABu8; 32]));
    let custody = Custody::derive(root, &config.env, config.plc_directory_submit, purposes)
        .expect("a dev root");
    let pool = adapter_pg::lazy_pool(UNREACHABLE_DATABASE_URL).expect("a lazy pool");
    Runtime::wire(config, pool, custody).expect("the runtime wires")
}

/// A message for the MAC tests.
fn message() -> MacMessage {
    let parts: [&[u8]; 1] = [b"a node"];
    MacMessage::try_from(parts.as_slice()).expect("one short part encodes")
}

#[tokio::test]
async fn account_create_runtime_refuses_den_mac() {
    let runtime = wired(&[Purpose::DidCustody]);

    let tagged = runtime.den_mac.mac(MacUse::DenNode, &message()).await;

    let not_held = CustodyError::PurposeNotHeld {
        purpose: Purpose::DenMac,
    };
    assert_eq!(tagged.err(), Some(not_held));
}

#[tokio::test]
async fn a_runtime_with_the_den_purpose_tags() {
    let runtime = wired(&[Purpose::DenMac]);

    let tagged = runtime.den_mac.mac(MacUse::DenNode, &message()).await;

    assert!(tagged.is_ok(), "the Den's key is held");
}

#[tokio::test]
async fn the_runtime_handles_print_no_root_bytes() {
    let runtime = wired(&[Purpose::DidCustody, Purpose::DenMac]);

    let shown = format!("{:?}", runtime.den_mac);

    assert_eq!(shown, "ScopedMac { purpose: DenMac }");
    let root_forms = ["ab", "AB", "q6ur", "171"];
    for form in root_forms {
        assert!(!shown.contains(form), "{shown} holds {form}");
    }
}

#[tokio::test]
async fn connecting_with_a_refused_root_never_reaches_the_database() {
    let mut config = dev_config();
    config.env = Environment::PROD;
    let root = RootSecret::from(Zeroizing::new([0xABu8; 32]));

    let connecting = Runtime::connect_migrating(config, root, &[Purpose::DidCustody]);
    let refused = connecting
        .await
        .err()
        .expect("production refuses a config root");

    let message = refused.to_string();
    assert!(
        message.starts_with("runtime setup failed: refusing to boot in PROD"),
        "{message}"
    );
}
