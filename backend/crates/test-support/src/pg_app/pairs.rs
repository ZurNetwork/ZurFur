//! The pairs of not-found causes a read must answer alike on the den world: the Den's own
//! routes and commission's reads behind its check run them, and so does the suite that reruns
//! them under statement recording.

use domain::elements::commission::Visibility;

use super::{
    DenWorld, SeededCommission, absent_account, absent_commission, absent_file, absent_slot,
    absent_workflow,
};

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

/// The stranger's pairs through the den routes against `target`, a commission they take part
/// in nothing of: the commission itself, its `changelog`, and its file.
pub fn den_stranger_pairs(target: &SeededCommission) -> [MissPair; 3] {
    let absent = absent_commission();
    let commission = target.id;
    let file = target.file;
    let absent_file = absent_file();
    let commission_path = format!("/den/commissions/{commission}");
    let changelog_path = format!("/den/commissions/{commission}/changelog");
    let file_path = format!("/den/commissions/{commission}/attachments/{file}");
    let commission_pair = MissPair {
        label: "the commission",
        absent: format!("/den/commissions/{absent}"),
        not_yours: commission_path.clone(),
        home: commission_path,
    };
    let changelog_pair = MissPair {
        label: "its changelog",
        absent: format!("/den/commissions/{absent}/changelog"),
        not_yours: changelog_path.clone(),
        home: changelog_path,
    };
    let file_pair = MissPair {
        label: "its file",
        absent: format!("/den/commissions/{absent}/attachments/{absent_file}"),
        not_yours: file_path.clone(),
        home: file_path,
    };
    [commission_pair, changelog_pair, file_pair]
}

/// The stranger's pair through the den routes at an Account: one no seed founds against
/// alice's "Supreme Arts".
pub fn den_account_pair(world: &DenWorld) -> MissPair {
    let absent = absent_account();
    let supreme_arts = format!("/den/accounts/{}", world.supreme_arts);
    MissPair {
        label: "an Account",
        absent: format!("/den/accounts/{absent}"),
        not_yours: supreme_arts.clone(),
        home: supreme_arts,
    }
}

/// The stranger's pair through the den routes under `characters`: a DID no seed writes
/// against bob's user DID. PostgreSQL stores no Character yet, so bob's DID stands in for one
/// that isn't the stranger's, and `home` is that same path, which no one opens yet.
pub fn den_character_pair() -> MissPair {
    let absent = absent_account();
    let bob = adapter_mem::fixture::bob();
    let bob_s_path = format!("/den/characters/{bob}");
    MissPair {
        label: "a character",
        absent: format!("/den/characters/{absent}"),
        not_yours: bob_s_path.clone(),
        home: bob_s_path,
    }
}

/// bob's pairs through the den routes as a Participant of "Untitled": a missing file key
/// against "Old sketch"'s real one under "Untitled", a missing slot against "Old sketch"'s,
/// and in his own Account's `workflows`, a missing Workflow against alice's Account's.
pub fn den_participant_pairs(world: &DenWorld) -> [MissPair; 3] {
    let untitled = world.untitled.id;
    let old_sketch = world.old_sketch.id;
    let night_shift = world.night_shift.to_string();
    let supreme_arts = world.supreme_arts.to_string();
    let foreign_file = world.old_sketch.file;
    let foreign_slot = world.old_sketch.slot;
    let foreign_workflow = world.queue;
    let absent_file = absent_file();
    let absent_slot = absent_slot();
    let absent_workflow = absent_workflow();
    let file_pair = MissPair {
        label: "a file key",
        absent: format!("/den/commissions/{untitled}/attachments/{absent_file}"),
        not_yours: format!("/den/commissions/{untitled}/attachments/{foreign_file}"),
        home: format!("/den/commissions/{old_sketch}/attachments/{foreign_file}"),
    };
    let slot_pair = MissPair {
        label: "a slot",
        absent: format!("/den/commissions/{untitled}/slots/{absent_slot}"),
        not_yours: format!("/den/commissions/{untitled}/slots/{foreign_slot}"),
        home: format!("/den/commissions/{old_sketch}/slots/{foreign_slot}"),
    };
    let workflow_pair = MissPair {
        label: "a Workflow",
        absent: format!("/den/accounts/{night_shift}/workflows/{absent_workflow}"),
        not_yours: format!("/den/accounts/{night_shift}/workflows/{foreign_workflow}"),
        home: format!("/den/accounts/{supreme_arts}/workflows/{foreign_workflow}"),
    };
    [file_pair, slot_pair, workflow_pair]
}

/// The live levels a stranger can reach "Untitled"'s node at, or a card of it, and each of
/// commission's reads' pairs there with the problem code both causes answer: the changelog's
/// `changelog` stays closed at both; the download opens the node at Public, so its pair is an
/// absent key against the real one under it, and refuses it at Listed.
pub fn commission_read_live_pairs(world: &DenWorld) -> [(Visibility, MissPair, &'static str); 4] {
    let [changelog, download] = commission_read_stranger_pairs(&world.untitled);
    let public_download = public_live_download_pair(&world.untitled);
    [
        (Visibility::Public, changelog.clone(), COMMISSION_NOT_FOUND),
        (Visibility::Public, public_download, FILE_NOT_FOUND),
        (Visibility::Listed, changelog, COMMISSION_NOT_FOUND),
        (Visibility::Listed, download, COMMISSION_NOT_FOUND),
    ]
}

/// The problem code of a commission the viewer can't open, on either of commission's reads.
pub const COMMISSION_NOT_FOUND: &str = "commission_not_found";

/// The problem code of a file the viewer can't open under a commission they can.
pub const FILE_NOT_FOUND: &str = "file_not_found";
