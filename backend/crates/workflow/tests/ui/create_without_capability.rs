//! A commission create can't be called without the den's write gate's create capability.

use identity::elements::user::UserId;
use kernel::elements::{did::Did, node::NodeId};
use workflow::elements::commission::{CommissionBirth, NewCommission};
use workflow::ports::CommissionWrites;

fn creates(writes: &mut impl CommissionWrites) {
    let birth = CommissionBirth {
        node: NodeId::mint(),
        owner_id: UserId::from(Did::from("did:plc:alice".to_owned())),
        created_at: chrono::Utc::now(),
    };
    let chosen = NewCommission {
        maturity: None,
        deadline: None,
    };
    let _write = writes.create(&birth, &chosen);
}

fn main() {}
