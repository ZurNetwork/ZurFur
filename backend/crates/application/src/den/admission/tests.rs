use super::*;

const A: Admission = Admission::Admit;
const R: Admission = Admission::Refuse;

/// One row of the table: a recorded state, what opened the node and at which level, and the
/// answers for a listing without and with deleted nodes, then for a path without and with.
type Row = (Option<NodeState>, Via, Level, [Admission; 4]);

const LIVE: Option<NodeState> = Some(NodeState::Live);
const ARCHIVED: Option<NodeState> = Some(NodeState::Archived);
const DEACTIVATED_OWNER: Option<NodeState> = Some(NodeState::Deactivated {
    viewer_is_owner: true,
});
const DEACTIVATED_OTHER: Option<NodeState> = Some(NodeState::Deactivated {
    viewer_is_owner: false,
});
const UNRECORDED: Option<NodeState> = None;

/// Every combination, with the answer the table of archived and deactivated nodes gives.
const TABLE: [Row; 30] = [
    // Live: always kept.
    (LIVE, Via::Standing, Level::Listed, [A, A, A, A]),
    (LIVE, Via::Standing, Level::Public, [A, A, A, A]),
    (LIVE, Via::Grant, Level::Listed, [A, A, A, A]),
    (LIVE, Via::Grant, Level::Public, [A, A, A, A]),
    (LIVE, Via::OwnLevel, Level::Listed, [A, A, A, A]),
    (LIVE, Via::OwnLevel, Level::Public, [A, A, A, A]),
    // Archived: a card is refused; opened by the viewer's own standing or grant, it opens on a
    // path and lists only when deleted nodes are asked for; opened by its own level alone, refused.
    (ARCHIVED, Via::Standing, Level::Listed, [R, R, R, R]),
    (ARCHIVED, Via::Standing, Level::Public, [R, A, A, A]),
    (ARCHIVED, Via::Grant, Level::Listed, [R, R, R, R]),
    (ARCHIVED, Via::Grant, Level::Public, [R, A, A, A]),
    (ARCHIVED, Via::OwnLevel, Level::Listed, [R, R, R, R]),
    (ARCHIVED, Via::OwnLevel, Level::Public, [R, R, R, R]),
    // Deactivated, the viewer its Owner: opens on a path, lists only when asked for.
    (
        DEACTIVATED_OWNER,
        Via::Standing,
        Level::Listed,
        [R, A, A, A],
    ),
    (
        DEACTIVATED_OWNER,
        Via::Standing,
        Level::Public,
        [R, A, A, A],
    ),
    (DEACTIVATED_OWNER, Via::Grant, Level::Listed, [R, A, A, A]),
    (DEACTIVATED_OWNER, Via::Grant, Level::Public, [R, A, A, A]),
    (
        DEACTIVATED_OWNER,
        Via::OwnLevel,
        Level::Listed,
        [R, A, A, A],
    ),
    (
        DEACTIVATED_OWNER,
        Via::OwnLevel,
        Level::Public,
        [R, A, A, A],
    ),
    // Deactivated, anyone else: refused.
    (
        DEACTIVATED_OTHER,
        Via::Standing,
        Level::Listed,
        [R, R, R, R],
    ),
    (
        DEACTIVATED_OTHER,
        Via::Standing,
        Level::Public,
        [R, R, R, R],
    ),
    (DEACTIVATED_OTHER, Via::Grant, Level::Listed, [R, R, R, R]),
    (DEACTIVATED_OTHER, Via::Grant, Level::Public, [R, R, R, R]),
    (
        DEACTIVATED_OTHER,
        Via::OwnLevel,
        Level::Listed,
        [R, R, R, R],
    ),
    (
        DEACTIVATED_OTHER,
        Via::OwnLevel,
        Level::Public,
        [R, R, R, R],
    ),
    // No record: refused.
    (UNRECORDED, Via::Standing, Level::Listed, [R, R, R, R]),
    (UNRECORDED, Via::Standing, Level::Public, [R, R, R, R]),
    (UNRECORDED, Via::Grant, Level::Listed, [R, R, R, R]),
    (UNRECORDED, Via::Grant, Level::Public, [R, R, R, R]),
    (UNRECORDED, Via::OwnLevel, Level::Listed, [R, R, R, R]),
    (UNRECORDED, Via::OwnLevel, Level::Public, [R, R, R, R]),
];

#[test]
fn every_combination_answers_as_the_table_says() {
    for (state, via, level, expected) in TABLE {
        let access = Access { level, via };
        let asked = [
            (Reach::Listing, false),
            (Reach::Listing, true),
            (Reach::Path, false),
            (Reach::Path, true),
        ];

        let answers =
            asked.map(|(reach, include_deleted)| admission(state, access, reach, include_deleted));

        assert_eq!(answers, expected, "{state:?} via {via:?} at {level:?}");
    }
}
