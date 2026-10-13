use application::den::{DenName, DenType, NodeKind, NodeLevel, PageToken, SoftDelete};
use serde_json::json;

use super::*;

fn name(text: &str) -> DenName {
    DenName::try_from(text).expect("a valid name")
}

fn node(access: DenAccess) -> den::DenNode {
    den::DenNode {
        segments: Vec::new(),
        name: name("Old sketch"),
        den_type: DenType::Commission,
        mount: true,
        access,
    }
}

#[test]
fn an_open_node_carries_its_words() {
    let access = DenAccess::Open {
        kind: NodeKind::Directory,
        own_level: NodeLevel::Listed,
        content_not_shown: true,
        soft_delete: Some(SoftDelete::Archived),
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
        own_level: NodeLevel::Private,
        content_not_shown: false,
        soft_delete: None,
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
