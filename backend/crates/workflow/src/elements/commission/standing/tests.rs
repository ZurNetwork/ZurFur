use kernel::elements::text::StoredText;

use super::*;
use crate::elements::commission::CommissionId;

const OWNER: Option<CommissionRole> = Some(CommissionRole::Owner);
const PARTICIPANT: Option<CommissionRole> = Some(CommissionRole::Participant);
const NO_ROLE: Option<CommissionRole> = None;
const ARCHIVED: bool = true;
const LIVE: bool = false;

/// Each row: the viewer's role, the commission's level, whether it is archived, and whether
/// the viewer is shown it. Written out, so a change to the rule fails a row.
const SHOWN_TABLE: [(Option<CommissionRole>, Level, bool, bool); 18] = [
    (OWNER, Level::Private, LIVE, true),
    (OWNER, Level::Private, ARCHIVED, true),
    (OWNER, Level::Listed, LIVE, true),
    (OWNER, Level::Listed, ARCHIVED, true),
    (OWNER, Level::Public, LIVE, true),
    (OWNER, Level::Public, ARCHIVED, true),
    (PARTICIPANT, Level::Private, LIVE, true),
    (PARTICIPANT, Level::Private, ARCHIVED, true),
    (PARTICIPANT, Level::Listed, LIVE, true),
    (PARTICIPANT, Level::Listed, ARCHIVED, true),
    (PARTICIPANT, Level::Public, LIVE, true),
    (PARTICIPANT, Level::Public, ARCHIVED, true),
    (NO_ROLE, Level::Private, LIVE, false),
    (NO_ROLE, Level::Private, ARCHIVED, false),
    (NO_ROLE, Level::Listed, LIVE, true),
    (NO_ROLE, Level::Listed, ARCHIVED, false),
    (NO_ROLE, Level::Public, LIVE, true),
    (NO_ROLE, Level::Public, ARCHIVED, false),
];

// A role always shows the commission; without one, only a live Listed or Public one shows.
#[test]
fn the_shown_rule_answers_every_row_of_its_table() {
    for (role, level, archived, expected) in SHOWN_TABLE {
        let standing = CommissionStanding {
            role,
            level,
            archived,
        };
        assert_eq!(
            standing.is_shown(),
            expected,
            "role {role:?}, level {level:?}, archived {archived}"
        );
    }
}

// The viewed summary's standing carries its role, its summary's level and its archived flag.
#[test]
fn a_viewed_summary_s_standing_reads_its_role_level_and_archive() {
    let summary = CommissionSummary {
        id: CommissionId::from(uuid::Uuid::now_v7()),
        title: StoredText::from("A commission".to_owned()),
        visibility: Level::Listed,
        archived: true,
    };
    let viewed = ViewedSummary {
        summary,
        role: PARTICIPANT,
    };
    let expected = CommissionStanding {
        role: PARTICIPANT,
        level: Level::Listed,
        archived: true,
    };
    assert_eq!(CommissionStanding::from(&viewed), expected);
}
