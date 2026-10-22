use super::*;

/// A thing whose text no capability's `Debug` may print.
const PLANTED: &str = "did:plc:planted-thing";

#[test]
fn debug_prints_nothing_of_the_thing() {
    let capability = WriteCapability::<String, Creator>::gate_issued(PLANTED.to_owned());

    let printed = format!("{capability:?}");

    assert_eq!(printed, "WriteCapability { .. }");
}

#[test]
fn thing_is_the_issued_thing() {
    let capability = WriteCapability::<String, Creator>::gate_issued(PLANTED.to_owned());

    assert_eq!(capability.thing(), PLANTED);
}

#[test]
fn new_reads_back_the_thing_it_wraps() {
    let new_thing = New::from(PLANTED.to_owned());

    let read: &String = new_thing.as_ref();

    assert_eq!(read, PLANTED);
}
