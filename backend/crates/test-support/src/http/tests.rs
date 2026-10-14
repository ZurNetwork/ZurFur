use std::sync::Arc;

use axum::{http::StatusCode, routing::get};
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
