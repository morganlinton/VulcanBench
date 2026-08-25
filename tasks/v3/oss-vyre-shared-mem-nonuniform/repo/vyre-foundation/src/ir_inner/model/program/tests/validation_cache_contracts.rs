use super::*;

#[test]
fn validation_cache_is_bound_to_current_fingerprint() {
    let mut program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    program.mark_validated_on("backend-a-test");
    assert!(program.is_validated_on("backend-a-test"));

    program.set_parallel_region_size([2, 1, 1]);
    assert!(
        !program.is_validated_on("backend-a-test"),
        "Fix: backend validation cache entries must be invalidated when Program fingerprint changes."
    );
}

#[test]
fn structural_validation_cache_is_cleared_by_parallel_region_mutation() {
    let mut program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    program
        .validate()
        .expect("Fix: valid fixture must pass structural validation");
    assert!(program.is_structurally_validated());

    program.set_parallel_region_size([0, 1, 1]);
    assert!(
        !program.is_structurally_validated(),
        "Fix: set_parallel_region_size must clear structural validation state."
    );
    assert!(
        program.validate().is_err(),
        "Fix: validation must re-run after parallel region mutation and reject zero dimensions."
    );
}

#[test]
fn structural_validation_cache_fails_closed_after_direct_field_mutation() {
    let mut program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    program
        .validate()
        .expect("Fix: valid fixture must pass structural validation");
    assert!(program.is_structurally_validated());

    program.workgroup_size = [0, 1, 1];
    assert!(
        !program.is_structurally_validated(),
        "Fix: structural validation skip-cache must compare against current Program bytes, not only an atomic flag."
    );
    assert!(
        program.validate().is_err(),
        "Fix: direct Program field mutation must not reuse stale structural validation."
    );
}

#[test]
fn backend_validation_cache_key_uses_current_program_bytes() {
    let mut program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    program.mark_validated_on("backend-a-test");
    assert!(program.is_validated_on("backend-a-test"));

    program.workgroup_size = [2, 1, 1];
    assert!(
        !program.is_validated_on("backend-a-test"),
        "Fix: backend validation cache keys must bind to current Program bytes even when a public field was mutated directly."
    );
}

#[test]
fn unknown_mutation_provenance_rejects_validation_and_backend_marking() {
    let mut program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    program
        .validate()
        .expect("Fix: valid fixture must pass structural validation before unknown mutation");
    program.mark_unknown_mutation_provenance();

    assert_eq!(
        program.validation_mutation_provenance(),
        super::super::meta::ProgramMutationProvenance::Unknown
    );
    assert!(
        !program.is_structurally_validated(),
        "Fix: unknown mutation provenance must clear structural validation state."
    );
    let error = program
        .validate()
        .expect_err("Fix: unknown mutation provenance must reject validation");
    assert!(
        error.to_string().contains("unknown mutation provenance"),
        "Fix: unknown mutation rejection must name the provenance failure: {error}"
    );

    program.mark_validated_on("backend-a-test");
    assert!(
        !program.is_validated_on("backend-a-test"),
        "Fix: unknown mutation provenance must prevent backend validation cache marking."
    );
}

