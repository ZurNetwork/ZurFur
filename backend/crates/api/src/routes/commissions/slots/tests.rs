//! Pins the `201` body's wire shape: `{"ids": ["<uuid>", …]}`.

use super::*;

#[test]
fn declare_slots_response_serializes_to_a_bare_ids_array() {
    let first = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000004").unwrap();
    let second = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000005").unwrap();
    let body = DeclareSlotsResponse {
        ids: vec![first, second],
    };

    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        format!("{{\"ids\":[\"{first}\",\"{second}\"]}}")
    );
}
