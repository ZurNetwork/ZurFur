//! The pairs of not-found causes a read must answer alike on the den world: the Den's own
//! routes and commission's reads behind its check run them, and so can any suite that reruns
//! them under statement recording.

use domain::elements::commission::Visibility;

use super::{DenWorld, SeededCommission, absent_commission, absent_file};

/// Two causes of one miss at the same depth: a path to nothing, and a path to a real node
/// that isn't the viewer's to open. Each must answer the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissPair {
    /// What the pair is, for assertion messages.
    pub label: &'static str,
    /// The path to nothing.
    pub absent: String,
    /// The path to a real node the viewer may not open.
    pub not_yours: String,
    /// The same real node at its home, where alice opens it.
    pub home: String,
}

/// The commissions the stranger's pairs run against, each at the visibility the pair needs:
/// "Untitled" Private and live, then the archived "Old sketch" Private, Public and Listed.
#[expect(
    clippy::let_and_return,
    reason = "the style rulebook names a multi-line construction before returning it"
)]
pub fn stranger_targets(world: &DenWorld) -> [(SeededCommission, Visibility); 4] {
    let targets = [
        (world.untitled, Visibility::Private),
        (world.old_sketch, Visibility::Private),
        (world.old_sketch, Visibility::Public),
        (world.old_sketch, Visibility::Listed),
    ];
    targets
}

/// The stranger's pairs through commission's two reads against `target`, a commission they
/// take part in nothing of: `GET /commissions/{id}/changelog` and
/// `GET /commissions/{id}/files/{key}`, each an absent commission against `target`.
pub fn commission_read_stranger_pairs(target: &SeededCommission) -> [MissPair; 2] {
    let absent = absent_commission();
    let absent_file = absent_file();
    let commission = target.id;
    let file = target.file;
    let changelog_path = format!("/commissions/{commission}/changelog");
    let file_path = format!("/commissions/{commission}/files/{file}");
    let changelog_pair = MissPair {
        label: "the changelog read",
        absent: format!("/commissions/{absent}/changelog"),
        not_yours: changelog_path.clone(),
        home: changelog_path,
    };
    let download_pair = MissPair {
        label: "the download",
        absent: format!("/commissions/{absent}/files/{absent_file}"),
        not_yours: file_path.clone(),
        home: file_path,
    };
    [changelog_pair, download_pair]
}

/// bob's pair as a Participant of "Untitled" through the download: a missing file key
/// against "Old sketch"'s real one, both under "Untitled".
pub fn download_participant_pair(world: &DenWorld) -> MissPair {
    let untitled = world.untitled.id;
    let old_sketch = world.old_sketch.id;
    let foreign_file = world.old_sketch.file;
    let absent_file = absent_file();
    MissPair {
        label: "the download of a file key",
        absent: format!("/commissions/{untitled}/files/{absent_file}"),
        not_yours: format!("/commissions/{untitled}/files/{foreign_file}"),
        home: format!("/commissions/{old_sketch}/files/{foreign_file}"),
    }
}

/// The stranger's pair through the download under `target` once it is Public and live, where
/// the commission's node opens to them and its `attachments` doesn't: an absent key against
/// `target`'s real one.
pub fn public_live_download_pair(target: &SeededCommission) -> MissPair {
    let commission = target.id;
    let file = target.file;
    let absent_file = absent_file();
    let file_path = format!("/commissions/{commission}/files/{file}");
    MissPair {
        label: "the download under a Public, live commission",
        absent: format!("/commissions/{commission}/files/{absent_file}"),
        not_yours: file_path.clone(),
        home: file_path,
    }
}
