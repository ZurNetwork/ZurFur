//! Compile-fail cases: each `ui/*.rs` must be rejected, with the compiler's message recorded in
//! the sibling `.stderr`.

#[test]
fn ui() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
