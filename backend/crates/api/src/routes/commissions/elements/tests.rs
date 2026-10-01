//! Pins the `201` body's wire shape: `{"id": "<uuid>"}`.

use super::*;

#[test]
fn add_element_response_serializes_to_a_bare_id_object() {
    let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000001").unwrap();
    let body = AddElementResponse { id };

    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        format!("{{\"id\":\"{id}\"}}")
    );
}
