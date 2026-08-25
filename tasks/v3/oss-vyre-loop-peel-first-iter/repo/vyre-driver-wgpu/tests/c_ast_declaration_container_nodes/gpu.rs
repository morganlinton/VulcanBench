// Integration test module for the containing Vyre package.

use super::fixtures::*;
use super::support::{
    cpu_gpu_classified, node_count_from_vast, run_gpu_classifier, run_gpu_vast_builder,
    typed_indices, word_at, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds,
    C_AST_KIND_BIT_FIELD_DECL, C_AST_KIND_ENUM_DECL, C_AST_KIND_FUNCTION_DEFINITION,
    C_AST_KIND_STATIC_ASSERT_DECL, C_AST_KIND_STRUCT_DECL, C_AST_KIND_TYPEDEF_DECL,
    C_AST_KIND_UNION_DECL,
};
use vyre_primitives::predicate::node_kind;

#[test]
fn gpu_parity_vast_builder_struct_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_definition();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for struct definition"
    );
}

#[test]
fn gpu_parity_vast_builder_union_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_definition();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for union definition"
    );
}

#[test]
fn gpu_parity_vast_builder_enum_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_definition();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for enum definition"
    );
}

#[test]
fn gpu_parity_vast_builder_function_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_definition();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for function definition"
    );
}

#[test]
fn gpu_parity_vast_builder_bitfield() {
    let (tok_types, tok_starts, tok_lens) = fixture_bitfield();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for bitfield"
    );
}

#[test]
fn gpu_parity_vast_builder_static_assert() {
    let (tok_types, tok_starts, tok_lens) = fixture_static_assert();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for _Static_assert"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests  -  classifier
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_classifier_struct_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for struct definition"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_STRUCT_DECL),
        vec![0],
        "GPU must classify struct keyword as STRUCT_DECL"
    );
}

#[test]
fn gpu_parity_classifier_union_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for union definition"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_UNION_DECL),
        vec![0],
        "GPU must classify union keyword as UNION_DECL"
    );
}

#[test]
fn gpu_parity_classifier_enum_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for enum definition"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_ENUM_DECL),
        vec![0],
        "GPU must classify enum keyword as ENUM_DECL"
    );
}

#[test]
fn gpu_parity_classifier_typedef_declaration() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_declaration();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for typedef declaration"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_TYPEDEF_DECL),
        vec![0],
        "GPU must classify typedef keyword as TYPEDEF_DECL"
    );
}

#[test]
fn gpu_parity_classifier_function_definition() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for function definition"
    );
    assert_eq!(
        word_at(&gpu, VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DEFINITION,
        "GPU must classify function name with body as FUNCTION_DEFINITION"
    );
}

#[test]
fn gpu_parity_classifier_function_prototype() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_prototype();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for function prototype"
    );
    assert_eq!(
        word_at(&gpu, VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "GPU must classify function name in prototype as FUNCTION_DECL"
    );
}

#[test]
fn gpu_parity_classifier_bitfield() {
    let (tok_types, tok_starts, tok_lens) = fixture_bitfield();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(gpu, expected, "GPU classifier must match CPU for bitfield");
    assert_eq!(
        word_at(&gpu, 3 * VAST_STRIDE_U32),
        C_AST_KIND_BIT_FIELD_DECL,
        "GPU must classify named bitfield identifier as BIT_FIELD_DECL"
    );
    assert_eq!(
        word_at(&gpu, 9 * VAST_STRIDE_U32),
        C_AST_KIND_BIT_FIELD_DECL,
        "GPU must classify unnamed bitfield colon as BIT_FIELD_DECL"
    );
}

#[test]
fn gpu_parity_classifier_static_assert() {
    let (tok_types, tok_starts, tok_lens) = fixture_static_assert();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for _Static_assert"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_STATIC_ASSERT_DECL),
        vec![0],
        "GPU must classify _Static_assert keyword as STATIC_ASSERT_DECL"
    );
}

// ---------------------------------------------------------------------------
// Combined CPU/GPU fast-path tests (no separate reference dispatch)
// ---------------------------------------------------------------------------

#[test]
fn cpu_gpu_forward_declarations_match_container_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_forward_declaration();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0],
        "struct forward declaration must classify as STRUCT_DECL"
    );

    let (tok_types, tok_starts, tok_lens) = fixture_union_forward_declaration();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![0],
        "union forward declaration must classify as UNION_DECL"
    );

    let (tok_types, tok_starts, tok_lens) = fixture_enum_forward_declaration();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ENUM_DECL),
        vec![0],
        "enum forward declaration must classify as ENUM_DECL"
    );
}

#[test]
fn cpu_gpu_function_definition_distinct_from_prototype() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_definition();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DEFINITION,
        "function definition name must be FUNCTION_DEFINITION"
    );

    let (tok_types, tok_starts, tok_lens) = fixture_function_prototype();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "function prototype name must be FUNCTION_DECL"
    );
}
