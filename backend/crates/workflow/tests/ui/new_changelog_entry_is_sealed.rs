//! A changelog entry can't be built from outside the domain: its fields are private, so a
//! gated kind is built only by its own constructor.

use identity::elements::user::UserId;
use kernel::elements::did::Did;
use workflow::elements::commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry};

fn main() {
    let _forged = NewChangelogEntry {
        commission_id: CommissionId::mint(),
        kind: ChangelogEntryKind::Renamed,
        actor_id: Some(UserId::from(Did::from("did:plc:alice".to_owned()))),
        payload: serde_json::json!({ "title": "A secret sketch" }),
        note: None,
        created_at: chrono::Utc::now(),
    };
}
