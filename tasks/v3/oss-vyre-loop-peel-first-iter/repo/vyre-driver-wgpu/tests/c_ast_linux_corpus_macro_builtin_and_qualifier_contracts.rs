//! Linux/kernel-grade C AST contracts for macro shapes, builtins, and qualifiers.
//!
//! Constructs under test:
//!   * container_of macro definition as preprocessor token stream + call-shaped usage
//!   * list_entry / list_for_each macro patterns
//!   * __builtin_expect (likely/unlikely) direct usage and macro wrapper preservation
//!   * static inline __attribute__((always_inline)) function definitions
//!   * volatile / _Atomic qualifier promotions in declarations and parameters
//!   * Linux error-label cleanup patterns (goto err; ... err: return -errno;)
//!
//! Every fixture asserts full GPU/CPU parity for build, annotate, and classify.
//! PG preservation is asserted for rows that carry semantic payload.
//! A missing GPU adapter is a configuration failure  -  tests panic loudly.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, assert_pg_preserves_row, c_fixture, kind_at, lexeme_indices,
    node_count_from_vast, row_indices, run_gpu_pg_lower_with_count as run_gpu_pg_lower,
    token_indices_containing, Fixture,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ALIGNOF_EXPR,
    C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT, C_AST_KIND_LABEL_STMT, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_RETURN_STMT,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// #define container_of(ptr, type, member) \
///   ({ const typeof(((type *)0)->member) *__mptr = (ptr); \
///      (type *)((char *)__mptr - offsetof(type, member)); })
/// struct node n;
/// struct node *p = container_of(&n.member, struct node, member);
/// ```
fn fixture_container_of_macro_and_use() -> Fixture {
    c_fixture![
        (
            "#define container_of(ptr, type, member) ({ const typeof(((type *)0)->member) *__mptr = (ptr); (type *)((char *)__mptr - offsetof(type, member)); })\n",
            TOK_PREPROC,
        ),
        ("struct", TOK_IDENTIFIER),
        ("node", TOK_IDENTIFIER),
        ("n", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("struct", TOK_IDENTIFIER),
        ("node", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("container_of", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("&", TOK_AMP),
        ("n", TOK_IDENTIFIER),
        (".", TOK_DOT),
        ("member", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("struct", TOK_IDENTIFIER),
        ("node", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("member", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// #define list_entry(ptr, type, member) container_of(ptr, type, member)
/// struct list_head head;
/// struct task_struct *t = list_entry(head.next, struct task_struct, tasks);
/// ```
fn fixture_list_entry_macro_and_use() -> Fixture {
    c_fixture![
        (
            "#define list_entry(ptr, type, member) container_of(ptr, type, member)\n",
            TOK_PREPROC,
        ),
        ("struct", TOK_IDENTIFIER),
        ("list_head", TOK_IDENTIFIER),
        ("head", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("struct", TOK_IDENTIFIER),
        ("task_struct", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("t", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("list_entry", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("head", TOK_IDENTIFIER),
        (".", TOK_DOT),
        ("next", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("struct", TOK_IDENTIFIER),
        ("task_struct", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("tasks", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// int r = __builtin_expect(!!(x), 1);
/// int s = __builtin_expect(!!(y), 0);
/// ```
fn fixture_builtin_expect_direct() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("r", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__builtin_expect", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("!", TOK_BANG),
        ("!", TOK_BANG),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("1", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("s", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__builtin_expect", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("!", TOK_BANG),
        ("!", TOK_BANG),
        ("(", TOK_LPAREN),
        ("y", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("0", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// #define likely(x) __builtin_expect(!!(x), 1)
/// #define unlikely(x) __builtin_expect(!!(x), 0)
/// int a = likely(cond);
/// int b = unlikely(cond);
/// ```
fn fixture_likely_unlikely_macro_shapes() -> Fixture {
    c_fixture![
        (
            "#define likely(x) __builtin_expect(!!(x), 1)\n",
            TOK_PREPROC,
        ),
        (
            "#define unlikely(x) __builtin_expect(!!(x), 0)\n",
            TOK_PREPROC,
        ),
        ("int", TOK_IDENTIFIER),
        ("a", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("likely", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("cond", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("b", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("unlikely", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("cond", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// static inline __attribute__((always_inline)) int dispatch(void) { return 0; }
/// static __attribute__((noinline)) void slow(void) { }
/// ```
fn fixture_static_inline_with_attributes() -> Fixture {
    c_fixture![
        ("static", TOK_STATIC),
        ("inline", TOK_INLINE),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("always_inline", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("int", TOK_IDENTIFIER),
        ("dispatch", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("return", TOK_RETURN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("static", TOK_STATIC),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("noinline", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("void", TOK_IDENTIFIER),
        ("slow", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// ```c
/// void probe(volatile unsigned long *flags, _Atomic unsigned long *state);
/// ```
fn fixture_volatile_atomic_parameters() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("probe", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("volatile", TOK_VOLATILE),
        ("unsigned", TOK_IDENTIFIER),
        ("long", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("flags", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("_Atomic", TOK_ATOMIC),
        ("unsigned", TOK_IDENTIFIER),
        ("long", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("state", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// int n = _Alignof(unsigned long);
/// ```
fn fixture_alignof_initializer_expression() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("n", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("_Alignof", TOK_ALIGNOF),
        ("(", TOK_LPAREN),
        ("unsigned", TOK_IDENTIFIER),
        ("long", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// int alloc(struct device *dev) {
///     if (!dev)
///         goto err_free;
///     return 0;
/// err_free:
///     kfree(dev);
///     return -1;
/// }
/// ```
fn fixture_linux_error_label_cleanup() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("alloc", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("struct", TOK_IDENTIFIER),
        ("device", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("dev", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("!", TOK_BANG),
        ("dev", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("goto", TOK_GOTO),
        ("err_free", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("return", TOK_RETURN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("err_free", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("kfree", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("dev", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("return", TOK_RETURN),
        ("-", TOK_MINUS),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]
}

// ---------------------------------------------------------------------------
// Tests  -  container_of
// ---------------------------------------------------------------------------

mod c_ast_linux_corpus_macro_builtin_and_qualifier_contracts_part1 {

    include!("__split/c_ast_linux_corpus_macro_builtin_and_qualifier_contracts_part1.rs");
}
mod c_ast_linux_corpus_macro_builtin_and_qualifier_contracts_part2 {
    include!("__split/c_ast_linux_corpus_macro_builtin_and_qualifier_contracts_part2.rs");
}
