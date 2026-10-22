//! A `WriteCapability` can't be built by struct literal: only the den's write gate issues one.

use std::marker::PhantomData;

use kernel::ports::{Creator, New, WriteCapability};

fn main() {
    let thing = New::from(7_u32);
    let _forged: WriteCapability<New<u32>, Creator> = WriteCapability {
        thing,
        role: PhantomData,
    };
}
