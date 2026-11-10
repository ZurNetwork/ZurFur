//! A `WriteCapability` can't be made by `From`: issuing one is a named call, banned by path.

use kernel::ports::{Creator, WriteCapability};

fn main() {
    let _forged = <WriteCapability<u32, Creator> as From<u32>>::from(7_u32);
}
