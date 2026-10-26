//! An open entry can't carry a gated kind: `event` takes only an open kind.

use identity::elements::user::UserId;
use kernel::elements::did::Did;
use workflow::elements::commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry};

fn main() {
    let _forged = NewChangelogEntry::event(
        CommissionId::mint(),
        ChangelogEntryKind::Renamed,
        UserId::from(Did::from("did:plc:alice".to_owned())),
        serde_json::json!({ "title": "A secret sketch" }),
        chrono::Utc::now(),
    );
}
