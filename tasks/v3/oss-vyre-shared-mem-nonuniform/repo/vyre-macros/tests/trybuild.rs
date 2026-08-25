//! External proc-macro UI tests for downstream compile behavior.

#[test]
fn proc_macro_ui_contracts() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/pass/skip_builder_passthrough.rs");
    cases.compile_fail("tests/ui/algebraic_laws_rejects_union.rs");
    cases.compile_fail("tests/ui/bad_ast_duplicate_enum.rs");
    cases.compile_fail("tests/ui/bad_ast_duplicate_variant.rs");
    cases.compile_fail("tests/ui/bad_define_op_input_type.rs");
    cases.compile_fail("tests/ui/bad_define_op_duplicate_argument.rs");
    cases.compile_fail("tests/ui/bad_define_op_unknown_argument.rs");
    cases.compile_fail("tests/ui/bad_pass_analyze_mode.rs");
    cases.compile_fail("tests/ui/bad_pass_boundary_class.rs");
    cases.compile_fail("tests/ui/bad_pass_cost_model_family.rs");
    cases.compile_fail("tests/ui/bad_pass_duplicate_invalidates.rs");
    cases.compile_fail("tests/ui/bad_pass_duplicate_argument.rs");
    cases.compile_fail("tests/ui/bad_pass_duplicate_requires.rs");
    cases.compile_fail("tests/ui/bad_pass_duplicate_requires_caps.rs");
    cases.compile_fail("tests/ui/bad_pass_missing_name.rs");
    cases.compile_fail("tests/ui/bad_pass_named_struct.rs");
    cases.compile_fail("tests/ui/bad_pass_on_enum.rs");
    cases.compile_fail("tests/ui/bad_pass_phase.rs");
    cases.compile_fail("tests/ui/bad_pass_preserves_abi_non_bool.rs");
    cases.compile_fail("tests/ui/bad_pass_requires_non_string.rs");
    cases.compile_fail("tests/ui/bad_pass_tuple_struct.rs");
    cases.compile_fail("tests/ui/bad_pass_unknown_argument.rs");
    cases.compile_fail("tests/ui/define_op_requires_program.rs");
    cases.compile_fail("tests/ui/derive_laws_on_union.rs");
    cases.compile_fail("tests/ui/derive_laws_unknown_attribute.rs");
    cases.compile_fail("tests/ui/derive_laws_unknown_law_variant.rs");
    cases.compile_fail("tests/ui/missing_define_op_program.rs");
    cases.compile_fail("tests/ui/vyre_pass_rejects_non_unit_struct.rs");
    cases.compile_fail("tests/ui/vyre_pass_rejects_duplicate_requires.rs");
}
