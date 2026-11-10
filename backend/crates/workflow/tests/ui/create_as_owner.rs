//! A commission create can't be called with an `AsOwner` capability: only a `Creator`'s
//! capability over a new commission's id creates one.

use identity::elements::user::UserId;
use kernel::elements::{did::Did, node::NodeId};
use kernel::ports::{New, WriteCapability};
use workflow::elements::commission::{AsOwner, CommissionBirth, CommissionId, NewCommission};
use workflow::ports::CommissionWrites;

fn creates(writes: &mut impl CommissionWrites) {
    let capability =
        WriteCapability::<New<CommissionId>, AsOwner>::gate_issued(New::from(CommissionId::mint()));
    let birth = CommissionBirth {
        node: NodeId::mint(),
        owner_id: UserId::from(Did::from("did:plc:alice".to_owned())),
        created_at: chrono::Utc::now(),
    };
    let chosen = NewCommission {
        maturity: None,
        deadline: None,
    };
    let _write = writes.create(&capability, &birth, &chosen);
}

fn main() {}
