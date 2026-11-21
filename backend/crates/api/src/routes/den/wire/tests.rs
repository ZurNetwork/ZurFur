use application::den::{DenName, DenType, NameOrdinal, NodeKind, PageToken, SoftDelete};
use domain::elements::{account::AccountId, commission::CommissionId, did::Did, level::Level};
use serde_json::json;
use uuid::Uuid;

use super::*;

fn name(text: &str) -> DenName {
    DenName::try_from(text).expect("a valid name")
}

fn node(access: DenAccess) -> den::DenNode {
    den::DenNode {
        segments: Vec::new(),
        name: name("Old sketch"),
        name_ordinal: None,
        den_type: DenType::Commission,
        mount: true,
        access,
    }
}

#[test]
fn an_open_node_carries_its_words() {
    let access = DenAccess::Open {
        kind: NodeKind::Directory,
        own_level: Level::Listed,
        content_not_shown: true,
        soft_delete: Some(SoftDelete::Archived),
        thing: None,
    };
    let expected = json!({
        "name": "Old sketch",
        "type": "commission",
        "kind": "directory",
        "mount": true,
        "open": {
            "ownLevel": "listed",
            "contentNotShown": true,
            "softDelete": "archived",
        },
    });

    let wire = serde_json::to_value(DenNode::from(node(access))).expect("serializes");

    assert_eq!(wire, expected);
}

#[test]
fn a_live_node_carries_no_soft_deleted_word() {
    let access = DenAccess::Open {
        kind: NodeKind::File,
        own_level: Level::Private,
        content_not_shown: false,
        soft_delete: None,
        thing: None,
    };
    let expected = json!({ "ownLevel": "private" });

    let wire = serde_json::to_value(DenNode::from(node(access))).expect("serializes");

    assert_eq!(wire["kind"], "file");
    assert_eq!(wire["open"], expected);
}

#[test]
fn a_card_carries_no_kind_and_an_empty_card() {
    let expected = json!({
        "name": "Old sketch",
        "type": "commission",
        "mount": true,
        "card": {},
    });

    let wire = serde_json::to_value(DenNode::from(node(DenAccess::Card))).expect("serializes");

    assert_eq!(wire, expected);
}

#[test]
fn the_last_page_carries_no_token() {
    let listing = den::DenListing::default();

    let wire = serde_json::to_value(DenListing::from(listing)).expect("serializes");

    assert_eq!(wire, json!({}));
}

#[test]
fn a_page_with_more_carries_its_token() {
    let token = "0100000000000003e8"
        .parse::<PageToken>()
        .expect("a well-formed token");
    let listing = den::DenListing {
        entries: Vec::new(),
        next_page: Some(token),
    };

    let wire = DenListing::from(listing);

    assert_eq!(wire.next_page_token, "0100000000000003e8");
}

#[test]
fn a_file_s_content_is_an_empty_file() {
    let content = get_den_node_response::Content::from(DenContent::File);

    assert_eq!(content, get_den_node_response::Content::File(DenFile {}));
}

/// Every key anywhere in `value`, at any depth.
fn keys(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Object(object) => object
            .iter()
            .flat_map(|(key, inner)| std::iter::once(key.clone()).chain(keys(inner)))
            .collect(),
        serde_json::Value::Array(items) => items.iter().flat_map(keys).collect(),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => Vec::new(),
    }
}

/// An open, live, Private directory standing for `thing`.
fn open_standing_for(thing: DenThing) -> DenAccess {
    DenAccess::Open {
        kind: NodeKind::Directory,
        own_level: Level::Private,
        content_not_shown: false,
        soft_delete: None,
        thing: Some(thing),
    }
}

#[test]
fn an_open_commission_carries_its_id() {
    let commission = CommissionId::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_0001));
    let access = open_standing_for(DenThing::Commission(commission));
    let expected = json!({
        "ownLevel": "private",
        "commissionId": "01a00000-0000-7000-8000-000000000001",
    });

    let wire = serde_json::to_value(DenNode::from(node(access))).expect("serializes");

    assert_eq!(wire["open"], expected);
}

#[test]
fn an_open_account_carries_its_did() {
    let account = AccountId::from(Did::from("did:plc:supreme-arts".to_owned()));
    let access = open_standing_for(DenThing::Account(account));
    let expected = json!({
        "ownLevel": "private",
        "accountDid": "did:plc:supreme-arts",
    });

    let wire = serde_json::to_value(DenNode::from(node(access))).expect("serializes");

    assert_eq!(wire["open"], expected);
}

#[test]
fn a_card_carries_no_thing() {
    let wire = serde_json::to_value(DenNode::from(node(DenAccess::Card))).expect("serializes");

    let keys = keys(&wire);

    assert!(!keys.iter().any(|key| key == "commissionId"), "{wire}");
    assert!(!keys.iter().any(|key| key == "accountDid"), "{wire}");
}

#[test]
fn a_crumb_carries_its_type() {
    let crumb = den::DenCrumb {
        segments: Vec::new(),
        name: name("Alice"),
        name_ordinal: None,
        den_type: DenType::User,
    };
    let expected = json!({ "name": "Alice", "type": "user" });

    let wire = serde_json::to_value(DenCrumb::from(crumb)).expect("serializes");

    assert_eq!(wire, expected);
}

/// A number that tells a name apart.
fn ordinal(number: u32) -> NameOrdinal {
    NameOrdinal::try_from(number).expect("a valid test ordinal")
}

#[test]
fn a_node_carries_its_name_ordinal() {
    let numbered = den::DenNode {
        name_ordinal: Some(ordinal(2)),
        ..node(DenAccess::Card)
    };

    let wire = serde_json::to_value(DenNode::from(numbered)).expect("serializes");

    assert_eq!(wire["nameOrdinal"], 2);
}

#[test]
fn a_crumb_carries_its_name_ordinal() {
    let crumb = den::DenCrumb {
        segments: Vec::new(),
        name: name("Test 1"),
        name_ordinal: Some(ordinal(3)),
        den_type: DenType::Commission,
    };
    let expected = json!({ "name": "Test 1", "type": "commission", "nameOrdinal": 3 });

    let wire = serde_json::to_value(DenCrumb::from(crumb)).expect("serializes");

    assert_eq!(wire, expected);
}
