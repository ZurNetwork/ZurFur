//! A `WriteCapability` can't be cloned: one issued capability is one leave.

use kernel::ports::{Creator, WriteCapability};

fn main() {
    let issued = WriteCapability::<u32, Creator>::gate_issued(7);
    let _copy = issued.clone();
}
