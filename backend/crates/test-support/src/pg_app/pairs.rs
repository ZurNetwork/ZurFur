//! The pairs of not-found causes a read must answer alike on the den world: the Den's own
//! routes and commission's reads behind its check run them, and so does the suite that reruns
//! them under statement recording.

use super::{
    DenWorld, SeededCommission, absent_account, absent_commission, absent_file, absent_slot,
    absent_workflow, other_absent_commission,
};
use domain::elements::{commission::CommissionId, level::Level, workflow::ColumnId};
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

/// What a call does to a commission, which says what the suites expect of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum CallKind {
    /// A read: `GET` the commission, its changelog or one of its files.
    Read,
    /// The board card's placement.
    Placement,
    /// Any other call: a write, a delete, an upload, a grant.
    Call,
}

/// What a call sends as its body.
#[derive(Debug, Clone, PartialEq)]
pub enum CallBody {
    /// No body.
    Empty,
    /// A JSON body its route's own parsing accepts.
    Json(serde_json::Value),
    /// A multipart body with one small PNG as its `file` part.
    Upload,
}

/// The placeholder in a call's tail where its route names a file key: the made-up key on a
/// made-up commission, the target's own on a real one.
const FILE_PLACEHOLDER: &str = "{file}";

/// One call on a commission, sent the same way to a real commission and to a made-up id.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionCall {
    /// What the call is, for assertion messages.
    pub label: &'static str,
    /// What the call does to the commission.
    pub kind: CallKind,
    /// The HTTP method.
    pub method: reqwest::Method,
    /// The path after `/commissions/{id}`: empty, or starting with `/`; any id it names is
    /// made up, and `{file}` stands where the route names a file key.
    pub tail: String,
    /// The body the call sends.
    pub body: CallBody,
}

impl CommissionCall {
    /// The call's path on `commission`, with the made-up file key where it names one.
    pub fn path(&self, commission: CommissionId) -> String {
        let tail = self
            .tail
            .replace(FILE_PLACEHOLDER, &absent_file().to_string());
        format!("/commissions/{commission}{tail}")
    }

    /// The call's path on `target`, with `target`'s own file key where it names one.
    pub fn path_on(&self, target: &SeededCommission) -> String {
        let tail = self
            .tail
            .replace(FILE_PLACEHOLDER, &target.file.to_string());
        let commission = target.id;
        format!("/commissions/{commission}{tail}")
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
            CallBody::Empty => request,
            CallBody::Json(body) => request.json(body),
            CallBody::Upload => request.multipart(upload_form()),
        };
        request.send().await.expect("the call is answered")
    }

    /// The pair of this call on `target`, a commission its viewer takes part in nothing of,
    /// against a made-up id. `home` is the real one's path, which alice may call.
    pub fn pair(&self, target: &SeededCommission) -> MissPair {
        let real = self.path_on(target);
        MissPair {
            label: self.label,
            absent: self.path(absent_commission()),
            not_yours: real.clone(),
            home: real,
        }
    }

    /// The pair of this call on two made-up ids, which must answer alike whoever asks:
    /// `not_yours` is the second made-up id, and `home` the first's path.
    pub fn absent_pair(&self) -> MissPair {
        let first = self.path(absent_commission());
        MissPair {
            label: self.label,
            absent: first.clone(),
            not_yours: self.path(other_absent_commission()),
            home: first,
        }
    }
}

/// The multipart form of the upload: one small PNG named `probe.png`.
fn upload_form() -> reqwest::multipart::Form {
    let part = reqwest::multipart::Part::bytes(b"a probed upload".to_vec())
        .file_name("probe.png")
        .mime_str("image/png")
        .expect("a valid content type");
    reqwest::multipart::Form::new().part("file", part)
}

/// The deadline every deadline call sends: far enough ahead that no sweep owes it.
const FAR_DEADLINE: &str = "2099-03-01T00:00:00Z";

/// The title of the one slot the declare call sends.
const DECLARED_SLOT_TITLE: &str = "A made-up slot";

/// A tab id no commission holds, for the bodies that name a tab: the route accepts it, and
/// authority answers before any tab is looked up.
fn absent_tab() -> uuid::Uuid {
    uuid::Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0001)
}

/// A call of `kind` whose tail names no file key.
fn call_of(
    label: &'static str,
    kind: CallKind,
    method: reqwest::Method,
    tail: impl Into<String>,
    body: CallBody,
) -> CommissionCall {
    CommissionCall {
        label,
        kind,
        method,
        tail: tail.into(),
        body,
    }
}

/// The rename, `PATCH` with [`RENAME_TITLE`].
pub fn rename_call() -> CommissionCall {
    let body = json!({ "title": RENAME_TITLE });
    call_of(
        "rename",
        CallKind::Call,
        reqwest::Method::PATCH,
        "/title",
        CallBody::Json(body),
    )
}

/// The eleven write calls the commission's field acts and the slot declare add, each sent with a
/// body its own parsing accepts: move the lifecycle, set and clear the direction, set and clear
/// the deadline, set and clear Delayed, raise and clear the dispute, the owner's rating, and
/// declare a slot.
pub fn new_write_calls() -> [CommissionCall; 11] {
    let move_body = json!({ "to": "active" });
    let direction_body = json!({ "status": "waiting_for_input" });
    let deadline_body = json!({ "deadline": FAR_DEADLINE });
    let delayed_body = json!({ "status": "delayed" });
    let rating_body = json!({ "rating": "suggestive", "graphic": false });
    let declare_body = json!([{
        "tab": absent_tab(),
        "surface": "content",
        "title": DECLARED_SLOT_TITLE,
    }]);
    let move_lifecycle = call_of(
        "move the lifecycle",
        CallKind::Call,
        reqwest::Method::PUT,
        "/lifecycle",
        CallBody::Json(move_body),
    );
    let set_direction = call_of(
        "set the direction",
        CallKind::Call,
        reqwest::Method::PUT,
        "/status/direction",
        CallBody::Json(direction_body),
    );
    let clear_direction = call_of(
        "clear the direction",
        CallKind::Call,
        reqwest::Method::DELETE,
        "/status/direction",
        CallBody::Empty,
    );
    let set_deadline = call_of(
        "set the deadline",
        CallKind::Call,
        reqwest::Method::PUT,
        "/deadline",
        CallBody::Json(deadline_body),
    );
    let clear_deadline = call_of(
        "clear the deadline",
        CallKind::Call,
        reqwest::Method::DELETE,
        "/deadline",
        CallBody::Empty,
    );
    let set_delayed = call_of(
        "set Delayed",
        CallKind::Call,
        reqwest::Method::PUT,
        "/status/deadline",
        CallBody::Json(delayed_body),
    );
    let clear_delayed = call_of(
        "clear Delayed",
        CallKind::Call,
        reqwest::Method::DELETE,
        "/status/deadline",
        CallBody::Empty,
    );
    let raise_dispute = call_of(
        "raise a dispute",
        CallKind::Call,
        reqwest::Method::PUT,
        "/dispute",
        CallBody::Empty,
    );
    let clear_dispute = call_of(
        "clear the dispute",
        CallKind::Call,
        reqwest::Method::DELETE,
        "/dispute",
        CallBody::Empty,
    );
    let rate = call_of(
        "rate",
        CallKind::Call,
        reqwest::Method::PUT,
        "/maturity",
        CallBody::Json(rating_body),
    );
    let declare_slots = call_of(
        "declare slots",
        CallKind::Call,
        reqwest::Method::POST,
        "/slots",
        CallBody::Json(declare_body),
    );
    [
        move_lifecycle,
        set_direction,
        clear_direction,
        set_deadline,
        clear_deadline,
        set_delayed,
        clear_delayed,
        raise_dispute,
        clear_dispute,
        rate,
        declare_slots,
    ]
}

/// The commission read, `GET /commissions/{id}`.
pub fn read_call() -> CommissionCall {
    call_of(
        "the commission read",
        CallKind::Read,
        reqwest::Method::GET,
        "",
        CallBody::Empty,
    )
}

/// The changelog read, `GET /commissions/{id}/changelog`.
pub fn changelog_read_call() -> CommissionCall {
    call_of(
        "the changelog read",
        CallKind::Read,
        reqwest::Method::GET,
        "/changelog",
        CallBody::Empty,
    )
}

/// The download, `GET /commissions/{id}/files/{key}`.
pub fn download_call() -> CommissionCall {
    let tail = format!("/files/{FILE_PLACEHOLDER}");
    call_of(
        "the download",
        CallKind::Read,
        reqwest::Method::GET,
        tail,
        CallBody::Empty,
    )
}

/// The board card's placement, `POST /commissions/{id}/placements` into `column`, at the
/// top of it.
pub fn placement_call(column: ColumnId) -> CommissionCall {
    let body = json!({ "column_id": column.to_string(), "index": 0 });
    call_of(
        "place a card",
        CallKind::Placement,
        reqwest::Method::POST,
        "/placements",
        CallBody::Json(body),
    )
}

/// The fifteen calls beyond the rename and the eleven new writes, each sent with a body its
/// own parsing accepts: delete, note, channel link and unlink, archive and unarchive, view
/// grant and revoke, element add and remove, seat declare, invitation issue and revoke,
/// upload, and markup.
fn other_calls() -> [CommissionCall; 15] {
    let element = absent_slot();
    let seat = absent_slot().to_string();
    let account = absent_account();
    let invitee = adapter_mem::fixture::bob().to_string();
    let note_body = json!({ "note": "A made-up note" });
    let channel_body = json!({ "channel": "https://t.me/a-made-up-chat" });
    let grant_body = json!({ "target_user_id": invitee, "level": "presentation" });
    let revoke_body = json!({ "target_user_id": invitee });
    let element_body = json!({
        "tab": absent_tab(),
        "surface": "content",
        "type": "note",
    });
    let seat_body = json!({
        "tab": absent_tab(),
        "surface": "content",
        "kind": "Creator",
    });
    let invitation_body = json!({ "seat": seat, "user": invitee });
    let markup_body = json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5, "r": 0.1 } } });
    let delete = call_of(
        "delete",
        CallKind::Call,
        reqwest::Method::DELETE,
        "",
        CallBody::Empty,
    );
    let note = call_of(
        "write a note",
        CallKind::Call,
        reqwest::Method::POST,
        "/notes",
        CallBody::Json(note_body),
    );
    let link_channel = call_of(
        "link the channel",
        CallKind::Call,
        reqwest::Method::PUT,
        "/channel",
        CallBody::Json(channel_body),
    );
    let clear_channel = call_of(
        "clear the channel",
        CallKind::Call,
        reqwest::Method::DELETE,
        "/channel",
        CallBody::Empty,
    );
    let archive = call_of(
        "archive",
        CallKind::Call,
        reqwest::Method::POST,
        "/archive",
        CallBody::Empty,
    );
    let unarchive = call_of(
        "unarchive",
        CallKind::Call,
        reqwest::Method::POST,
        "/unarchive",
        CallBody::Empty,
    );
    let grant = call_of(
        "grant a view",
        CallKind::Call,
        reqwest::Method::POST,
        "/grants",
        CallBody::Json(grant_body),
    );
    let revoke = call_of(
        "revoke a view",
        CallKind::Call,
        reqwest::Method::DELETE,
        format!("/grants/{account}"),
        CallBody::Json(revoke_body),
    );
    let add_element = call_of(
        "add an element",
        CallKind::Call,
        reqwest::Method::POST,
        "/elements",
        CallBody::Json(element_body),
    );
    let remove_element = call_of(
        "remove an element",
        CallKind::Call,
        reqwest::Method::DELETE,
        format!("/elements/{element}"),
        CallBody::Empty,
    );
    let declare_seat = call_of(
        "declare a seat",
        CallKind::Call,
        reqwest::Method::POST,
        "/seats",
        CallBody::Json(seat_body),
    );
    let invite = call_of(
        "issue an invitation",
        CallKind::Call,
        reqwest::Method::POST,
        "/invitations",
        CallBody::Json(invitation_body.clone()),
    );
    let revoke_invitation = call_of(
        "revoke an invitation",
        CallKind::Call,
        reqwest::Method::DELETE,
        "/invitations",
        CallBody::Json(invitation_body),
    );
    let upload = call_of(
        "upload a file",
        CallKind::Call,
        reqwest::Method::POST,
        "/files",
        CallBody::Upload,
    );
    let markup = call_of(
        "mark up a file",
        CallKind::Call,
        reqwest::Method::POST,
        format!("/files/{FILE_PLACEHOLDER}/markup"),
        CallBody::Json(markup_body),
    );
    [
        delete,
        note,
        link_channel,
        clear_channel,
        archive,
        unarchive,
        grant,
        revoke,
        add_element,
        remove_element,
        declare_seat,
        invite,
        revoke_invitation,
        upload,
        markup,
    ]
}

/// Every route of the commissions router that names a commission id, every method of every
/// path (`POST /commissions` names none): the three reads, the board card's placement into
/// `column`, and the twenty-seven other calls, each with a body its own parsing accepts so the
/// answer is authority's.
pub fn every_commission_call(column: ColumnId) -> Vec<CommissionCall> {
    let mut calls = vec![
        read_call(),
        changelog_read_call(),
        download_call(),
        placement_call(column),
        rename_call(),
    ];
    calls.extend(new_write_calls());
    calls.extend(other_calls());
    calls
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
