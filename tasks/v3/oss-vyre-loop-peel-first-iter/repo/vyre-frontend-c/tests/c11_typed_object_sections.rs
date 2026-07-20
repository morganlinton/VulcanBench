//! Typed VAST and ProgramGraph sections in compiled C11 object artifacts.
mod support;

use support::*;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{
    C_AST_PG_CATEGORY_CONTROL, C_AST_PG_EDGE_GOTO_TARGET, C_AST_PG_EDGE_SWITCH_CASE,
    C_AST_PG_EDGE_SWITCH_DEFAULT, C_AST_PG_ROLE_GOTO, C_AST_PG_ROLE_RETURN,
};
use vyre_libs::parsing::c::parse::vast::{
    C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_CASE_STMT, C_AST_KIND_CONDITIONAL_EXPR,
    C_AST_KIND_DEFAULT_STMT, C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_RETURN_STMT, C_AST_KIND_SWITCH_STMT, C_EXPR_SHAPE_BINARY,
    C_EXPR_SHAPE_CONDITIONAL, C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;

#[test]
fn compiled_object_carries_typed_vast_program_graph_and_expression_shape() {
    let object = compile_source(
        "typed_kernel_libc_shaped",
        KERNEL_LIBC_SHAPED_SOURCE,
        vec![("CLI_DEFINED".to_string(), Some("13".to_string()))],
    );
    object.assert_elf();

    let lex = object.lex();
    let vast_words = object.words(SECTION_VAST);
    let pg_words = object.words(SECTION_PROGRAM_GRAPH);
    let semantic_pg_nodes = object.words(SECTION_SEMANTIC_PROGRAM_GRAPH_NODES);
    let semantic_pg_edges = object.words(SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES);
    let expr_shape_words = object.words(SECTION_EXPRESSION_SHAPE);
    assert_typed_vast_and_pg_rows(
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        &vast_words,
        &pg_words,
    );
    assert_eq!(
        expr_shape_words.len(),
        lex.tok_types.len() * C_EXPR_SHAPE_STRIDE_U32 as usize
    );
    assert_eq!(
        semantic_pg_nodes.len(),
        lex.tok_types.len() * SEMANTIC_PG_NODE_STRIDE_U32
    );
    assert_eq!(
        semantic_pg_edges.len(),
        lex.tok_types.len() * SEMANTIC_PG_EDGE_ROWS_PER_NODE * SEMANTIC_PG_EDGE_STRIDE_U32
    );

    let switch_idx = find_kind(&lex.tok_types, TOK_SWITCH);
    assert_eq!(vast_kind(&vast_words, switch_idx), C_AST_KIND_SWITCH_STMT);
    let case_idx = find_kind_after(&lex.tok_types, TOK_CASE, switch_idx);
    assert_eq!(vast_kind(&vast_words, case_idx), C_AST_KIND_CASE_STMT);
    let default_idx = find_kind_after(&lex.tok_types, TOK_DEFAULT, case_idx);
    assert_eq!(vast_kind(&vast_words, default_idx), C_AST_KIND_DEFAULT_STMT);
    let if_idx = find_kind_after(&lex.tok_types, TOK_IF, default_idx);
    assert_eq!(vast_kind(&vast_words, if_idx), C_AST_KIND_IF_STMT);
    let goto_idx = find_kind_after(&lex.tok_types, TOK_GOTO, case_idx);
    assert_eq!(vast_kind(&vast_words, goto_idx), C_AST_KIND_GOTO_STMT);
    let goto_semantic = &semantic_pg_nodes
        [goto_idx * SEMANTIC_PG_NODE_STRIDE_U32..(goto_idx + 1) * SEMANTIC_PG_NODE_STRIDE_U32];
    assert_eq!(goto_semantic[6], C_AST_PG_CATEGORY_CONTROL);
    assert_eq!(goto_semantic[7], C_AST_PG_ROLE_GOTO);
    let return_idx = find_kind_after(&lex.tok_types, TOK_RETURN, goto_idx);
    assert_eq!(vast_kind(&vast_words, return_idx), C_AST_KIND_RETURN_STMT);
    let return_semantic = &semantic_pg_nodes
        [return_idx * SEMANTIC_PG_NODE_STRIDE_U32..(return_idx + 1) * SEMANTIC_PG_NODE_STRIDE_U32];
    assert_eq!(return_semantic[6], C_AST_PG_CATEGORY_CONTROL);
    assert_eq!(return_semantic[7], C_AST_PG_ROLE_RETURN);

    for edge_kind in [
        C_AST_PG_EDGE_GOTO_TARGET,
        C_AST_PG_EDGE_SWITCH_CASE,
        C_AST_PG_EDGE_SWITCH_DEFAULT,
    ] {
        assert!(
            semantic_pg_edges
                .chunks_exact(SEMANTIC_PG_EDGE_STRIDE_U32)
                .any(|row| row[0] == edge_kind),
            "semantic ProgramGraph edge kind {edge_kind} is present in object"
        );
    }

    let question_idx = find_kind(&lex.tok_types, TOK_QUESTION);
    assert_eq!(
        vast_kind(&vast_words, question_idx),
        C_AST_KIND_CONDITIONAL_EXPR
    );
    assert_eq!(
        expr_shape_words[question_idx * C_EXPR_SHAPE_STRIDE_U32 as usize],
        C_EXPR_SHAPE_CONDITIONAL,
        "object expression-shape section marks ternary operators"
    );

    let assign_idx = find_kind_before(&lex.tok_types, TOK_ASSIGN, question_idx);
    assert_eq!(vast_kind(&vast_words, assign_idx), C_AST_KIND_ASSIGN_EXPR);
    assert_eq!(
        expr_shape_words[assign_idx * C_EXPR_SHAPE_STRIDE_U32 as usize],
        C_EXPR_SHAPE_BINARY,
        "object expression-shape section marks assignment-shaped operators"
    );

    let dot_idx = find_kind(&lex.tok_types, TOK_DOT);
    assert_eq!(
        vast_kind(&vast_words, dot_idx),
        C_AST_KIND_MEMBER_ACCESS_EXPR
    );
    let arrow_idx = find_kind(&lex.tok_types, TOK_ARROW);
    assert_eq!(
        vast_kind(&vast_words, arrow_idx),
        C_AST_KIND_MEMBER_ACCESS_EXPR
    );
    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[0] == C_AST_KIND_FIELD_DECL),
        "object VAST preserves field declaration nodes"
    );
    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[0] == C_AST_KIND_POINTER_DECL),
        "object VAST preserves pointer declaration nodes"
    );
    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[0] == C_AST_KIND_FUNCTION_DECLARATOR),
        "object VAST preserves function declarator nodes"
    );

    for kind in [
        node_kind::FUNCTION_DECL,
        node_kind::CALL,
        node_kind::BASIC_BLOCK,
        node_kind::BINARY,
        node_kind::LITERAL,
        node_kind::VARIABLE,
    ] {
        assert!(
            vast_words
                .chunks_exact(VAST_STRIDE_U32)
                .any(|row| row[0] == kind),
            "object VAST carries typed node kind {kind}"
        );
        assert!(
            pg_words
                .chunks_exact(PG_STRIDE_U32)
                .any(|row| row[0] == kind),
            "object ProgramGraph carries typed node kind {kind}"
        );
    }

    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[TYPEDEF_FLAGS_FIELD] & TYPEDEF_DECLARATOR_FLAG != 0),
        "typedef declarator annotation survives into object VAST"
    );
    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[TYPEDEF_FLAGS_FIELD] & VISIBLE_TYPEDEF_FLAG != 0),
        "visible typedef-name annotation survives into object VAST"
    );
}
