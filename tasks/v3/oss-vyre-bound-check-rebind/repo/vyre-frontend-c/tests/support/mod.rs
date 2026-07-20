// Integration test module for the containing Vyre package.

#![allow(dead_code, unused_imports)]

pub(crate) mod ast_assert;
pub(crate) mod ast_oracle;
pub(crate) mod clang_abi;
pub(crate) mod clang_diagnostics;
pub(crate) mod clang_tokens;
pub(crate) mod fixtures;
pub(crate) mod gpu_preprocess;
pub(crate) mod object;
pub(crate) mod object_envelope;

pub(crate) use ast_assert::{
    assert_token_kind, assert_typed_vast_and_pg_rows, assert_vast_kind_and_span, find_kind,
    find_kind_after, find_kind_before, find_token, find_token_after, find_token_before,
    find_token_in_context, token_text, vast_kind,
};
pub(crate) use fixtures::{AST_PARSER_GAP_SOURCE, KERNEL_LIBC_SHAPED_SOURCE, SOURCE};
pub(crate) use object::{
    compile_source, compile_source_with_resident, parse_lex_section, parse_vyrecob2_section,
    parse_vyrecob2_sections, read_u32, u32_words_from_bytes, u32_words_to_bytes, CompiledObject,
    LexRows, MAGIC, ORDINARY_DECL_FLAG, PG_STRIDE_U32, SECTION_AST, SECTION_BRACE_PAIRS,
    SECTION_CALLS, SECTION_CFG, SECTION_EXPRESSION_SHAPE, SECTION_FUNCTIONS, SECTION_LEX,
    SECTION_MACRO_TYPES, SECTION_PAREN_PAIRS, SECTION_PREPROC_MASK, SECTION_PROGRAM_GRAPH,
    SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES, SECTION_SEMANTIC_PROGRAM_GRAPH_NODES, SECTION_SEMA_SCOPE,
    SECTION_VAST, SEMANTIC_PG_EDGE_ROWS_PER_NODE, SEMANTIC_PG_EDGE_STRIDE_U32,
    SEMANTIC_PG_NODE_STRIDE_U32, SEMA_STRIDE_U32, TYPEDEF_DECLARATOR_FLAG, TYPEDEF_FLAGS_FIELD,
    VAST_STRIDE_U32, VISIBLE_TYPEDEF_FLAG,
};
pub(crate) use object_envelope::{ObjectEnvelope, ObjectFlavor};
