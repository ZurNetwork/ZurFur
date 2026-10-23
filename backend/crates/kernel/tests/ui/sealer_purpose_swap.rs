//! A handle for sign-in's secrets can't be passed where the did:plc custody handle is wanted.

use std::sync::Arc;

use kernel::elements::custody::{ForDidCustody, ForSigninSecrets};
use kernel::ports::{KeyCustodian, ScopedSealer};

fn keep_did_custody(_sealer: ScopedSealer<ForDidCustody>) {}

fn swap(custodian: Arc<dyn KeyCustodian>) {
    let signin_secrets = ScopedSealer::<ForSigninSecrets>::from(custodian);
    keep_did_custody(signin_secrets);
}

fn main() {}
