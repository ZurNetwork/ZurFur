//! The pairs of not-found causes a read must answer alike on the den world: the Den's own
//! routes and commission's reads behind its check run them, and so does the suite that reruns
//! them under statement recording.

use super::{
    DenWorld, SeededCommission, absent_account, absent_commission, absent_file, absent_slot,
    absent_workflow,
};
use domain::elements::{commission::CommissionId, level::Level};
use serde_json::json;

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
pub fn stranger_targets(world: &DenWorld) -> [(SeededCommission, Level); 4] {
    let targets = [
        (world.untitled, Level::Private),
        (world.old_sketch, Level::Private),
        (world.old_sketch, Level::Public),
        (world.old_sketch, Level::Listed),
    ];
    targets
}

/// The targets of the stranger's den-route pairs: [`stranger_targets`], then "Untitled" live
/// at Public and at Listed.
pub fn den_stranger_targets(world: &DenWorld) -> [(SeededCommission, Level); 6] {
    let [first, second, third, fourth] = stranger_targets(world);
    [
        first,
        second,
        third,
        fourth,
        (world.untitled, Level::Public),
        (world.untitled, Level::Listed),
    ]
}

/// bob's pairs as a Participant of the archived "Old sketch", once
/// [`DenWorld::add_bob_to_old_sketch`] has made him one: through the den routes, a missing
/// file key against "Untitled"'s real one under "Old sketch" and a missing slot against
/// "Untitled"'s; through the download, a missing key against "Untitled"'s.
pub fn archived_participant_pairs(world: &DenWorld) -> [MissPair; 3] {
    let untitled = world.untitled.id;
    let old_sketch = world.old_sketch.id;
    let foreign_file = world.untitled.file;
    let foreign_slot = world.untitled.slot;
    let absent_file = absent_file();
    let absent_slot = absent_slot();
    let file_pair = MissPair {
        label: "a file key under an archived commission",
        absent: format!("/den/commissions/{old_sketch}/attachments/{absent_file}"),
        not_yours: format!("/den/commissions/{old_sketch}/attachments/{foreign_file}"),
        home: format!("/den/commissions/{untitled}/attachments/{foreign_file}"),
    };
    let slot_pair = MissPair {
        label: "a slot under an archived commission",
        absent: format!("/den/commissions/{old_sketch}/slots/{absent_slot}"),
        not_yours: format!("/den/commissions/{old_sketch}/slots/{foreign_slot}"),
        home: format!("/den/commissions/{untitled}/slots/{foreign_slot}"),
    };
    let download_pair = MissPair {
        label: "the download of a file key under an archived commission",
        absent: format!("/commissions/{old_sketch}/files/{absent_file}"),
        not_yours: format!("/commissions/{old_sketch}/files/{foreign_file}"),
        home: format!("/commissions/{untitled}/files/{foreign_file}"),
    };
    [file_pair, slot_pair, download_pair]
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

/// The title every rename pair sends: one the name rule keeps, so each answer is the
/// standing's.
pub const RENAME_TITLE: &str = "Bob's now";

/// bob's pair through the rename, `PATCH` with [`RENAME_TITLE`]: an absent commission against
/// alice's archived "Old sketch", which he takes no part in until
/// [`DenWorld::add_bob_to_old_sketch`]. `home` is the real one's path, which alice renames.
pub fn rename_stranger_pair(world: &DenWorld) -> MissPair {
    let absent = absent_commission();
    let old_sketch = format!("/commissions/{}/title", world.old_sketch.id);
    MissPair {
        label: "the rename",
        absent: format!("/commissions/{absent}/title"),
        not_yours: old_sketch.clone(),
        home: old_sketch,
    }
}

/// One call on a commission, sent the same way to a real commission and to a made-up id.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionCall {
    /// What the call is, for assertion messages.
    pub label: &'static str,
    /// The HTTP method.
    pub method: reqwest::Method,
    /// The path after `/commissions/{id}`: empty, or starting with `/`.
    pub tail: &'static str,
    /// The JSON body, when the call takes one: one the call's own parsing accepts, so the
    /// answer is the standing's.
    pub body: Option<serde_json::Value>,
}

impl CommissionCall {
    /// The call's path on `commission`.
    pub fn path(&self, commission: CommissionId) -> String {
        format!("/commissions/{commission}{}", self.tail)
    }

    /// The call as `client` sends it to `path` on the server at `base`.
    pub async fn send(
        &self,
        client: &reqwest::Client,
        base: &str,
        path: &str,
    ) -> reqwest::Response {
        let request = client.request(self.method.clone(), format!("{base}{path}"));
        let request = match &self.body {
            Some(body) => request.json(body),
            None => request,
        };
        request.send().await.expect("the call is answered")
    }

    /// The pair of this call on `target`, a commission its viewer takes part in nothing of,
    /// against a made-up id. `home` is the real one's path, which alice may call.
    pub fn pair(&self, target: &SeededCommission) -> MissPair {
        let real = self.path(target.id);
        MissPair {
            label: self.label,
            absent: self.path(absent_commission()),
            not_yours: real.clone(),
            home: real,
        }
    }
}

/// The deadline every deadline call sends: far enough ahead that no sweep owes it.
const FAR_DEADLINE: &str = "2099-03-01T00:00:00Z";

/// The rename, `PATCH` with [`RENAME_TITLE`].
pub fn rename_call() -> CommissionCall {
    let body = json!({ "title": RENAME_TITLE });
    CommissionCall {
        label: "rename",
        method: reqwest::Method::PATCH,
        tail: "/title",
        body: Some(body),
    }
}

/// The seven write calls the commission's field acts add, each sent with a body its own
/// parsing accepts: move the lifecycle, set and clear the direction, set and clear the
/// deadline, set and clear Delayed.
pub fn new_write_calls() -> [CommissionCall; 7] {
    let move_body = json!({ "to": "active" });
    let direction_body = json!({ "status": "waiting_for_input" });
    let deadline_body = json!({ "deadline": FAR_DEADLINE });
    let delayed_body = json!({ "status": "delayed" });
    let move_lifecycle = CommissionCall {
        label: "move the lifecycle",
        method: reqwest::Method::PUT,
        tail: "/lifecycle",
        body: Some(move_body),
    };
    let set_direction = CommissionCall {
        label: "set the direction",
        method: reqwest::Method::PUT,
        tail: "/status/direction",
        body: Some(direction_body),
    };
    let clear_direction = CommissionCall {
        label: "clear the direction",
        method: reqwest::Method::DELETE,
        tail: "/status/direction",
        body: None,
    };
    let set_deadline = CommissionCall {
        label: "set the deadline",
        method: reqwest::Method::PUT,
        tail: "/deadline",
        body: Some(deadline_body),
    };
    let clear_deadline = CommissionCall {
        label: "clear the deadline",
        method: reqwest::Method::DELETE,
        tail: "/deadline",
        body: None,
    };
    let set_delayed = CommissionCall {
        label: "set Delayed",
        method: reqwest::Method::PUT,
        tail: "/status/deadline",
        body: Some(delayed_body),
    };
    let clear_delayed = CommissionCall {
        label: "clear Delayed",
        method: reqwest::Method::DELETE,
        tail: "/status/deadline",
        body: None,
    };
    [
        move_lifecycle,
        set_direction,
        clear_direction,
        set_deadline,
        clear_deadline,
        set_delayed,
        clear_delayed,
    ]
}

/// The commission read, `GET /commissions/{id}`.
pub fn read_call() -> CommissionCall {
    CommissionCall {
        label: "the commission read",
        method: reqwest::Method::GET,
        tail: "",
        body: None,
    }
}

/// The live levels a stranger can reach "Untitled"'s node at, or a card of it, and each of
/// commission's reads' pairs there with the problem code both causes answer: the changelog's
/// `changelog` stays closed at both; the download opens the node at Public, so its pair is an
/// absent key against the real one under it, and refuses it at Listed.
pub fn commission_read_live_pairs(world: &DenWorld) -> [(Level, MissPair, &'static str); 4] {
    let [changelog, download] = commission_read_stranger_pairs(&world.untitled);
    let public_download = public_live_download_pair(&world.untitled);
    [
        (Level::Public, changelog.clone(), COMMISSION_NOT_FOUND),
        (Level::Public, public_download, FILE_NOT_FOUND),
        (Level::Listed, changelog, COMMISSION_NOT_FOUND),
        (Level::Listed, download, COMMISSION_NOT_FOUND),
    ]
}

/// The problem code of a commission the viewer can't open, on either of commission's reads.
pub const COMMISSION_NOT_FOUND: &str = "commission_not_found";

/// The problem code of a file the viewer can't open under a commission they can.
pub const FILE_NOT_FOUND: &str = "file_not_found";
