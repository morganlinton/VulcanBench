// Integration test module for the containing Vyre package.

use super::fixtures::*;
use super::support::{
    run_gpu_expr_shape, run_gpu_pg_lower, run_reference_pg_lower, starts_for_lens,
};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes,
    reference_c11_build_vast_nodes as reference_build_vast,
    reference_c11_classify_vast_node_kinds as reference_classify,
};

#[test]
fn gpu_matches_cpu_for_unary_prefix_and_postfix_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_unary_prefix_and_postfix();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for unary operators"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for unary operators"
    );
}

#[test]
fn gpu_matches_cpu_for_cast_expression_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_cast_expr();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for cast expression"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for cast expression"
    );
}

#[test]
fn gpu_matches_cpu_for_member_access_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_member_access();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for member access"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for member access"
    );
}

#[test]
fn gpu_matches_cpu_for_array_subscript_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_array_subscript();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for array subscript"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for array subscript"
    );
}

#[test]
fn gpu_matches_cpu_for_designated_initializer_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_designated_initializer();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for designated initializer"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for designated initializer"
    );
}

#[test]
fn gpu_matches_cpu_for_array_range_designator_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_array_range_designator();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for array range designator"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for array range designator"
    );
}

#[test]
fn gpu_matches_cpu_for_gnu_case_range_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = fixture_gnu_case_range();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_build_vast(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_classify(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU for GNU case range"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU for GNU case range"
    );
}
