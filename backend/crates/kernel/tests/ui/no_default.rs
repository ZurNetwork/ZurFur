//! A `WriteCapability` has no `Default`: there is no capability for nothing in particular.

use kernel::ports::{Creator, WriteCapability};

fn main() {
    let _forged = WriteCapability::<u32, Creator>::default();
}
