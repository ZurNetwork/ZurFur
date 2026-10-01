//! Pins the body's wire shape to the exact strings the retired
//! `json!({ "status": …, "database": … })` literals produced —
//! alphabetical key order, matching `serde_json`'s `BTreeMap`
//! (no `preserve_order`).

use super::*;

#[test]
fn health_response_serializes_the_ok_pair() {
    let body = HealthResponse {
        database: "up",
        status: "ok",
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"database":"up","status":"ok"}"#
    );
}

#[test]
fn health_response_serializes_the_degraded_pair() {
    let body = HealthResponse {
        database: "down",
        status: "degraded",
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"database":"down","status":"degraded"}"#
    );
}
