#![allow(missing_docs)]

#[test]
fn malformed_macro_inputs_fail_with_actionable_diagnostics() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
