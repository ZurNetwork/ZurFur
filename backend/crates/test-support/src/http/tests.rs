use std::sync::Arc;

use axum::{
    http::{HeaderMap, StatusCode},
    routing::get,
};
use domain::elements::did::Did;
use tokio::sync::Notify;
use tokio::time::Instant;

use super::*;
use crate::cut::{CUT_WAIT, DropWitness, send_raw};

#[tokio::test]
async fn the_served_rig_runs_a_cut_request_to_its_end() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let finished = Arc::new(Notify::new());
    let handler = {
        let (entered, release, finished) = (entered.clone(), release.clone(), finished.clone());
        move || async move {
            entered.notify_one();
            release.notified().await;
            finished.notify_one();
            StatusCode::NO_CONTENT
        }
    };
    let witness = DropWitness::default();
    let outer = witness.clone();
    let builder = crate::runtime::mem(&Did::from("did:plc:rig-cut".to_string()));
    let served = serve_wrapped(
        builder,
        move |_runtime| Router::new().route("/held", get(handler)),
        move |stacked| stacked.layer(outer),
    )
    .await;

    let connection = send_raw(
        &served.base_url,
        "GET /held HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    tokio::time::timeout(CUT_WAIT, entered.notified())
        .await
        .expect("the handler runs");
    drop(connection);
    witness.dropped().await;
    release.notify_one();

    tokio::time::timeout(CUT_WAIT, finished.notified())
        .await
        .expect("the cut request runs to its end");
    served
        .detach
        .drain(Instant::now() + CUT_WAIT)
        .await
        .expect("nothing is left running");
}

/// Serves a route echoing every value of the request's `Origin` and
/// `Sec-Fetch-Site` headers, one `name: value` line each.
async fn serve_signal_echo() -> Served {
    let builder = crate::runtime::mem(&Did::from("did:plc:rig-signal".to_string()));
    let echo = |headers: HeaderMap| async move {
        ["origin", "sec-fetch-site"]
            .into_iter()
            .flat_map(|name| {
                headers
                    .get_all(name)
                    .iter()
                    .map(move |value| format!("{name}: {}\n", value.to_str().unwrap_or("?")))
            })
            .collect::<String>()
    };
    serve(builder, move |_runtime| {
        Router::new().route("/signal", get(echo))
    })
    .await
}

/// The first-party signal lines `client` sends on a `GET /signal`.
async fn signal_sent(served: &Served, client: &reqwest::Client) -> String {
    let response = client
        .get(format!("{}/signal", served.base_url))
        .send()
        .await
        .expect("GET /signal");
    response.text().await.expect("the echo is text")
}

#[tokio::test]
async fn the_test_client_sends_same_origin_fetch_metadata() {
    let served = serve_signal_echo().await;
    let sent = signal_sent(&served, &client()).await;
    assert_eq!(sent, "sec-fetch-site: same-origin\n");
}

#[tokio::test]
async fn the_origin_client_sends_its_origin_alone() {
    let served = serve_signal_echo().await;
    let origin_client = client_with_origin(&served.base_url);
    let sent = signal_sent(&served, &origin_client).await;
    let expected = format!("origin: {}\n", served.base_url);
    assert_eq!(sent, expected);
}

#[tokio::test]
async fn the_bare_client_sends_no_signal() {
    let served = serve_signal_echo().await;
    let sent = signal_sent(&served, &bare_client()).await;
    assert_eq!(sent, "");
}

#[tokio::test]
async fn a_request_header_replaces_the_client_default() {
    let served = serve_signal_echo().await;
    let response = client()
        .get(format!("{}/signal", served.base_url))
        .header("sec-fetch-site", "cross-site")
        .send()
        .await
        .expect("GET /signal");
    let sent = response.text().await.expect("the echo is text");
    assert_eq!(sent, "sec-fetch-site: cross-site\n");
}
