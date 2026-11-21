use super::*;

/// A made-up node whose segment and name are easy to spot in printed text.
fn new_node() -> NewNode {
    NewNode {
        id: NodeId::mint(),
        parent: Some(NodeId::mint()),
        segment: NodeSegment::try_from("spotted-segment").expect("a segment"),
        node_type: NodeType::Slot,
        level: Level::Listed,
        name: StoredText::from("Spotted name".to_owned()),
    }
}

#[test]
fn debug_prints_neither_the_segment_nor_the_name() {
    let node = new_node();
    let row = NodeRow::from(node.clone());

    let printed = [format!("{node:?}"), format!("{row:?}")];

    for text in printed {
        assert!(!text.contains("spotted-segment"), "{text}");
        assert!(!text.contains("Spotted name"), "{text}");
        assert!(text.contains(&node.id.to_string()), "{text}");
    }
}

#[test]
fn a_new_node_reads_back_field_for_field() {
    let node = new_node();

    let row = NodeRow::from(node.clone());

    assert_eq!(row.id, node.id);
    assert_eq!(row.parent, node.parent);
    assert_eq!(row.segment, node.segment);
    assert_eq!(row.node_type, node.node_type);
    assert_eq!(row.level, node.level);
    assert_eq!(row.name, node.name);
}
