//! Pins the `201` body's wire shape, `{"ids": ["<uuid>", …]}`, and the field pointer each
//! refused title answers at.

use super::*;

#[test]
fn declare_slots_response_serializes_to_a_bare_ids_array() {
    let first = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000004").unwrap();
    let second = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000005").unwrap();
    let body = DeclareCommissionSlotsResponse {
        ids: vec![first.to_string(), second.to_string()],
    };

    let expected = format!("{{\"ids\":[\"{first}\",\"{second}\"]}}");
    assert_eq!(serde_json::to_string(&body).unwrap(), expected);
}

#[test]
fn a_refused_title_points_at_its_item_s_title() {
    let refused = slot_title(2, "   ").expect_err("a blank title is refused");

    let expected = FieldRefusal {
        pointer: FieldPointer::item_member(2, "title"),
        code: FieldCode::NameEmpty,
    };
    assert_eq!(refused, expected);
}
