//! A handler is lent a `&WriteCapability`: it can neither keep the capability nor return it.

use kernel::ports::{Creator, WriteCapability};

/// Somewhere a handler would like to stash the capability.
struct Keeper {
    kept: Option<WriteCapability<u32, Creator>>,
}

fn keeps(capability: &WriteCapability<u32, Creator>, keeper: &mut Keeper) {
    keeper.kept = Some(*capability);
}

fn returns(capability: &WriteCapability<u32, Creator>) -> WriteCapability<u32, Creator> {
    *capability
}

fn main() {
    let issued = WriteCapability::<u32, Creator>::gate_issued(7);
    let mut keeper = Keeper { kept: None };
    keeps(&issued, &mut keeper);
    let _returned = returns(&issued);
}
