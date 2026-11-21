//! The shared account-deletion suite (`test_support::account_deletion_contract`), run against
//! the in-memory stores: one fresh runtime per case, its commissions seeded through the minting
//! seeder.

mod common;

use std::sync::Arc;

use domain::elements::did::Did;
use test_support::account_deletion_contract::Runtime;
use test_support::seeder::CommissionSeeder;

/// A fresh in-memory runtime and the commission seeder.
async fn fresh() -> (Runtime, Arc<dyn CommissionSeeder>, ()) {
    let did = Did::from("did:plc:accountdeletion".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    (runtime, Arc::new(common::MintingSeeder), ())
}

test_support::account_deletion_contract_tests!(fresh);
