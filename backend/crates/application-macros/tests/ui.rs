//! Compile-fail cases: each `ui/*.rs` must be rejected with the macros' own
//! spanned message, recorded in the sibling `.stderr`; and pass cases: each
//! `pass/*.rs` compiles and its `main` runs green.

#[test]
fn ui() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
    cases.pass("tests/pass/*.rs");
}
