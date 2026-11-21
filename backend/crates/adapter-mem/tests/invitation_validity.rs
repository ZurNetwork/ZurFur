//! The shared invitation-validity suite (`test_support::invitation_validity_contract`), run
//! against the in-memory stores: one fresh runtime per case.

use domain::elements::did::Did;
use test_support::invitation_validity_contract::Runtime;

/// A fresh in-memory runtime.
async fn fresh() -> (Runtime, ()) {
    let did = Did::from("did:plc:invitationvalidity".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    (runtime, ())
}

test_support::invitation_validity_contract_tests!(fresh);
