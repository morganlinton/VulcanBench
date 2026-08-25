// Integration test module for the containing Vyre package.

use super::fixtures::*;
use super::support::{typed_indices, word_at, VAST_STRIDE_U32};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds,
    C_AST_KIND_BIT_FIELD_DECL, C_AST_KIND_ENUM_DECL, C_AST_KIND_FUNCTION_DEFINITION,
    C_AST_KIND_STATIC_ASSERT_DECL, C_AST_KIND_STRUCT_DECL, C_AST_KIND_TYPEDEF_DECL,
    C_AST_KIND_UNION_DECL,
};
use vyre_primitives::predicate::node_kind;

#[test]
fn cpu_struct_definition_classifies_struct_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0],
        "struct keyword in definition must classify as STRUCT_DECL"
    );
}

#[test]
fn cpu_struct_forward_declaration_classifies_struct_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_forward_declaration();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0],
        "struct keyword in forward declaration must classify as STRUCT_DECL"
    );
}

#[test]
fn cpu_union_definition_classifies_union_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![0],
        "union keyword in definition must classify as UNION_DECL"
    );
}

#[test]
fn cpu_union_forward_declaration_classifies_union_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_forward_declaration();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![0],
        "union keyword in forward declaration must classify as UNION_DECL"
    );
}

#[test]
fn cpu_enum_definition_classifies_enum_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ENUM_DECL),
        vec![0],
        "enum keyword in definition must classify as ENUM_DECL"
    );
}

#[test]
fn cpu_enum_forward_declaration_classifies_enum_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_forward_declaration();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ENUM_DECL),
        vec![0],
        "enum keyword in forward declaration must classify as ENUM_DECL"
    );
}

#[test]
fn cpu_typedef_declaration_classifies_typedef_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_declaration();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_TYPEDEF_DECL),
        vec![0],
        "typedef keyword must classify as TYPEDEF_DECL"
    );
}

#[test]
fn cpu_function_definition_classifies_name() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_definition();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DEFINITION,
        "function name with body must classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn cpu_function_prototype_classifies_name_as_function_decl() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_prototype();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "function name in prototype must classify as FUNCTION_DECL, not FUNCTION_DEFINITION"
    );
    assert_ne!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DEFINITION,
        "function prototype must not classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn cpu_bitfield_classifies_identifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_bitfield();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_BIT_FIELD_DECL,
        "named bitfield identifier must classify as BIT_FIELD_DECL"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::LITERAL,
        "named bitfield width must classify as literal"
    );
    assert_eq!(
        word_at(&typed, 9 * VAST_STRIDE_U32),
        C_AST_KIND_BIT_FIELD_DECL,
        "unnamed bitfield colon must classify as BIT_FIELD_DECL"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        node_kind::LITERAL,
        "unnamed bitfield width must classify as literal"
    );
}

#[test]
fn cpu_static_assert_classifies_keyword() {
    let (tok_types, tok_starts, tok_lens) = fixture_static_assert();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_STATIC_ASSERT_DECL),
        vec![0],
        "_Static_assert keyword must classify as STATIC_ASSERT_DECL"
    );
}
