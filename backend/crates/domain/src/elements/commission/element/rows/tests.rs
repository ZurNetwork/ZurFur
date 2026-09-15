use chrono::Utc;
use serde_json::json;

use super::*;
use crate::elements::did::Did;

// A new element's envelope: fresh id, address, acting user, payload
// verbatim, placeholder band — and no mode field to set.
#[test]
fn a_new_element_carries_its_address_and_payload() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let tab = TabId::mint();
    let surface = "content".parse::<SurfaceName>().expect("valid");
    let address = SurfaceAddress::new(tab, surface.clone());
    let element_type = "note".parse::<ElementType>().expect("valid");
    let owner = UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())));
    let body = json!({ "body": "Reference: 三毛猫 🐾", "revision": 3 });
    let payload = ElementPayload::from(body.clone());

    let contributed = NewElement::contributed(
        commission,
        address.clone(),
        element_type.clone(),
        payload.clone(),
        owner.clone(),
        Utc::now(),
    );

    assert_eq!(contributed.commission_id, commission);
    assert_eq!(contributed.address.tab, tab, "the tab is addressed BY ID");
    assert_eq!(contributed.address.surface, surface, "the surface too");
    assert_eq!(contributed.element_type, element_type);
    assert_eq!(
        contributed.payload, payload,
        "the payload is carried opaque"
    );
    assert_eq!(contributed.band, Band::default());
    assert_eq!(contributed.created_by, owner);

    // The satellite carrier shares an already-minted identity.
    let seat_id = ElementId::mint();
    let carrier = NewElement::carrying(
        seat_id,
        commission,
        address,
        ElementType::seat(),
        owner,
        Utc::now(),
    );
    assert_eq!(carrier.id, seat_id, "one identity, two rows");
    assert_eq!(
        carrier.payload.as_ref(),
        &json!({}),
        "substance lives in the satellite"
    );
    assert_eq!(carrier.band, Band::default());
}

// The payload's doors round-trip and the empty default is `{}`.
#[test]
fn the_payload_wraps_opaque_json_and_defaults_to_the_empty_object() {
    let raw = json!({ "list": [1, 2, 3], "nothing": null, "flag": true });
    let payload = ElementPayload::from(raw.clone());

    assert_eq!(
        payload.as_ref(),
        &raw,
        "carried verbatim, via the std borrow door"
    );
    assert_eq!(
        serde_json::Value::from(payload.clone()),
        raw,
        "and the owned one, via the std Into door"
    );
    assert_eq!(serde_json::Value::from(payload), raw, "as does From");

    assert_eq!(
        ElementPayload::default().as_ref(),
        &json!({}),
        "the empty default is `{{}}`, matching the jsonb column DEFAULT — \
         deliberately NOT serde_json's own `null` default"
    );
}
