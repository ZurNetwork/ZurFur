//! No free text rides a gated entry: only an open entry has `with_note`.

use identity::elements::user::UserId;
use kernel::elements::did::Did;
use workflow::elements::commission::{CommissionId, NewChangelogEntry};

fn main() {
    let actor = UserId::from(Did::from("did:plc:alice".to_owned()));
    let _noted = NewChangelogEntry::renamed(CommissionId::mint(), actor, chrono::Utc::now())
        .with_note("A secret sketch".to_owned());
}
