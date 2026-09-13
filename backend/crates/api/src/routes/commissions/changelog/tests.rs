//! Pins `ChangelogEntryBody.created_at`'s wire format.

use chrono::{TimeZone, Utc};

use super::*;

#[test]
fn changelog_entry_body_created_at_is_z_normalized() {
    let at = Utc.with_ymd_and_hms(2025, 7, 25, 12, 0, 0).unwrap();
    let body = ChangelogEntryBody {
        seq: 1,
        kind: "created",
        actor_id: None,
        payload: serde_json::json!({}),
        note: None,
        created_at: WireTimestamp::from(at),
    };

    let wire = serde_json::to_value(&body).unwrap();
    assert_eq!(wire["created_at"], "2025-07-25T12:00:00Z");
}
