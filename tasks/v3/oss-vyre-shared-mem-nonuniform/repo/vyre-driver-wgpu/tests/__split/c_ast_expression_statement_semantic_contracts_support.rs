// Deep semantic contract tests for C parser expressions and statements.
//
// Covers:
//   - cast expressions (simple, complex, vs parenthesized expressions)
//   - compound literals (simple, with designated initializer)
//   - C11 generic selection (_Generic with associations and default)
//   - sizeof / _Alignof (type-name and expression forms)
//   - conditional expressions (simple, nested ternary)
//   - comma expressions (boundaries, shapes in statement contexts)
//   - labels (consecutive, nested in control-flow bodies)
//   - goto (forward, backward, cross-block)
//   - switch / case / default (fallthrough, body grouping)
//   - GNU case ranges (case 1 ... 5: with RANGE_DESIGNATOR_EXPR)
//   - loops (for, while, do with break / continue)
//   - return (with and without expression, nested compound)
//   - GNU statement expressions (assignment context, nested, labels/goto inside)
//
// Every fixture asserts semantic VAST/AST invariants: kind classification,
// parent/child tree links, span preservation, expression-shape boundaries,
// and PG lowering preservation. GPU/CPU parity is asserted for the full pipeline.

// cfg(feature = "c-parser")  -  moved to parent

// Bring shared fixture helpers + libs items into the suite-mod scope so
// part1/part2 can reach them via `use super::*;`.
use crate::c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, word_at, Fixture, FixtureToken,
    VAST_STRIDE_U32,
};
use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL,
    C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_BREAK_STMT,
    C_AST_KIND_CASE_STMT, C_AST_KIND_CAST_EXPR, C_AST_KIND_COMPOUND_LITERAL_EXPR,
    C_AST_KIND_CONDITIONAL_EXPR, C_AST_KIND_CONTINUE_STMT, C_AST_KIND_DEFAULT_STMT,
    C_AST_KIND_DO_STMT, C_AST_KIND_FOR_STMT, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_GENERIC_ASSOCIATION, C_AST_KIND_GENERIC_SELECTION_EXPR, C_AST_KIND_GOTO_STMT,
    C_AST_KIND_IF_STMT, C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_LABEL_STMT,
    C_AST_KIND_RANGE_DESIGNATOR_EXPR, C_AST_KIND_RETURN_STMT, C_AST_KIND_SIZEOF_EXPR,
    C_AST_KIND_SWITCH_STMT, C_AST_KIND_UNARY_EXPR, C_AST_KIND_WHILE_STMT,
};
use vyre_primitives::predicate::node_kind;

