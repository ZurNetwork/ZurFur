//! ZMVP-103 acceptance tests: boot a throwaway PDS, act as a fixture account,
//! destroy it — two instances provably isolated, nothing reaching the public
//! atproto network. Requires a container runtime socket (DOCKER_HOST
//! honored), exactly like the Postgres-based suites.

use serde_json::{Value, json};
use test_support::{ActingCredential, FixtureAccount, ThrowawayPds};

/// Builds the Authorization value for a fixture credential.
///
/// This match is the forward-looking ZMVP-105 seam assertion: because
/// `ActingCredential` is `#[non_exhaustive]`, this (external) crate is forced
/// to keep a wildcard arm — adding an OAuth or other variant for 105's auth
/// fork later compiles without breaking any consumer.
fn bearer(account: &FixtureAccount) -> String {
    match &account.credential {
        ActingCredential::PdsSession { access_jwt, .. } => format!("Bearer {access_jwt}"),
        _ => unreachable!("future credential variants choose their own auth construction"),
    }
}

async fn xrpc_get(client: &reqwest::Client, url: String, auth: Option<&str>) -> (u16, Value) {
    let mut req = client.get(url);
    if let Some(auth) = auth {
        req = req.header("Authorization", auth);
    }
    let res = req.send().await.expect("xrpc request reaches the PDS");
    let status = res.status().as_u16();
    let body = res.json().await.unwrap_or(Value::Null);
    (status, body)
}

/// AC1 + AC2 + the 105 seam: boot a fresh empty PDS, provision a fixture
/// account, act as it over authenticated XRPC, and verify the container is
/// gone after drop.
#[tokio::test]
async fn boot_act_destroy() {
    let client = reqwest::Client::new();
    let pds = ThrowawayPds::boot().await.expect("throwaway PDS boots");
    let endpoint = pds.endpoint().to_string();

    // Booted and healthy.
    let (status, health) = xrpc_get(&client, format!("{endpoint}/xrpc/_health"), None).await;
    assert_eq!(status, 200, "health endpoint answers: {health}");
    assert!(health.get("version").is_some(), "health reports a version");

    // Provision: the seam carries endpoint + acting identity + credential.
    let account = pds
        .provision_account("actor.test")
        .await
        .expect("fixture account provisions");
    assert_eq!(account.endpoint, endpoint, "seam names the PDS it lives on");
    assert_eq!(account.handle, "actor.test");
    assert!(
        account.did.starts_with("did:plc:"),
        "fixture identity is a did:plc ({})",
        account.did
    );

    // Act as the fixture: an authenticated session round-trip...
    let auth = bearer(&account);
    let (status, session) = xrpc_get(
        &client,
        format!("{endpoint}/xrpc/com.atproto.server.getSession"),
        Some(&auth),
    )
    .await;
    assert_eq!(status, 200, "credential authenticates: {session}");
    assert_eq!(session["did"], json!(account.did));
    assert_eq!(session["handle"], json!(account.handle));

    // ...and a real record write/read against the fixture's own repo (the
    // surface ZMVP-105 binds to), via raw XRPC — rig proof, not adapter code.
    let put = client
        .post(format!("{endpoint}/xrpc/com.atproto.repo.putRecord"))
        .header("Authorization", &auth)
        .json(&json!({
            "repo": account.did,
            "collection": "app.zurfur.test.probe",
            "rkey": "rig-probe",
            "record": {"$type": "app.zurfur.test.probe", "createdAt": "2026-07-04T00:00:00Z"},
        }))
        .send()
        .await
        .expect("putRecord reaches the PDS");
    assert!(
        put.status().is_success(),
        "fixture can write to its own repo: {}",
        put.text().await.unwrap_or_default()
    );

    let (status, record) = xrpc_get(
        &client,
        format!(
            "{endpoint}/xrpc/com.atproto.repo.getRecord?repo={}&collection=app.zurfur.test.probe&rkey=rig-probe",
            account.did
        ),
        None,
    )
    .await;
    assert_eq!(status, 200, "written record reads back: {record}");
    assert_eq!(record["value"]["$type"], json!("app.zurfur.test.probe"));

    // The stub PLC serves a resolvable DID document (identity reads work too).
    let (status, description) = xrpc_get(
        &client,
        format!(
            "{endpoint}/xrpc/com.atproto.repo.describeRepo?repo={}",
            account.did
        ),
        None,
    )
    .await;
    assert_eq!(status, 200, "DID document resolves locally: {description}");
    assert_eq!(description["did"], json!(account.did));

    // Destroy: dropping the handle removes the container; the endpoint dies.
    drop(pds);
    let mut gone = false;
    for _ in 0..60 {
        match client
            .get(format!("{endpoint}/xrpc/_health"))
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
        {
            Err(_) => {
                gone = true;
                break;
            }
            Ok(_) => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
        }
    }
    assert!(
        gone,
        "container must be torn down on drop (endpoint still answers)"
    );
}

/// AC3: two independently booted PDSes share no state — the same handle
/// provisions on both as two different identities, and a record written on
/// one is invisible to the other.
#[tokio::test]
async fn two_instances_never_observe_each_others_state() {
    let client = reqwest::Client::new();
    let (a, b) = tokio::join!(ThrowawayPds::boot(), ThrowawayPds::boot());
    let (a, b) = (a.expect("PDS A boots"), b.expect("PDS B boots"));

    // The same handle is free on both: shared state would reject the second
    // registration as HandleNotAvailable.
    let on_a = a
        .provision_account("twin.test")
        .await
        .expect("handle provisions on A");
    let on_b = b
        .provision_account("twin.test")
        .await
        .expect("the same handle provisions independently on B");
    assert_ne!(on_a.did, on_b.did, "two instances mint two identities");

    // A record written on A does not exist on B.
    let put = client
        .post(format!("{}/xrpc/com.atproto.repo.putRecord", a.endpoint()))
        .header("Authorization", bearer(&on_a))
        .json(&json!({
            "repo": on_a.did,
            "collection": "app.zurfur.test.probe",
            "rkey": "only-on-a",
            "record": {"$type": "app.zurfur.test.probe", "createdAt": "2026-07-04T00:00:00Z"},
        }))
        .send()
        .await
        .expect("putRecord reaches A");
    assert!(put.status().is_success());

    let query = format!(
        "/xrpc/com.atproto.repo.getRecord?repo={}&collection=app.zurfur.test.probe&rkey=only-on-a",
        on_a.did
    );
    let (status_a, _) = xrpc_get(&client, format!("{}{query}", a.endpoint()), None).await;
    assert_eq!(status_a, 200, "the record exists where it was written");
    let (status_b, body_b) = xrpc_get(&client, format!("{}{query}", b.endpoint()), None).await;
    assert_ne!(
        status_b, 200,
        "B must not see A's repo (isolation broken): {body_b}"
    );

    // Neither instance published identities into the other's PLC surface.
    assert_eq!(a.published_plc_dids(), vec![on_a.did.clone()]);
    assert_eq!(b.published_plc_dids(), vec![on_b.did.clone()]);
}

/// AC4: the fixture flow completes with no public-network dependency — the
/// genesis operation for the minted identity provably landed on the
/// harness's own loopback stub PLC (the only directory the PDS knows).
#[tokio::test]
async fn provisioning_publishes_identity_to_the_local_stub_only() {
    let pds = ThrowawayPds::boot().await.expect("throwaway PDS boots");
    assert!(
        pds.published_plc_dids().is_empty(),
        "a fresh PDS has published nothing"
    );

    let account = pds
        .provision_account("hermit.test")
        .await
        .expect("provisioning succeeds against the local stub PLC");

    let published = pds.published_plc_dids();
    assert_eq!(
        published,
        vec![account.did.clone()],
        "the identity's genesis op landed on OUR stub — provisioning has no \
         path to the public plc.directory"
    );
}
