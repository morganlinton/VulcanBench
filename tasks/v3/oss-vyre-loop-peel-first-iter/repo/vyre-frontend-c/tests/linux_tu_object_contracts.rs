//! End-to-end object contracts for Linux-shaped translation units: resident TU preparation
//! (`#include` inlining, GPU-resident CLI `-D` macro state), lexer spans, and typed VAST / ProgramGraph structure.

mod support;

use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::object_decode::decode_object_sema_scope;

use support::{
    assert_typed_vast_and_pg_rows, compile_source_with_resident, find_kind, find_token_in_context,
    vast_kind, CompiledObject, PG_STRIDE_U32, SECTION_AST, SECTION_BRACE_PAIRS, SECTION_CALLS,
    SECTION_CFG, SECTION_EXPRESSION_SHAPE, SECTION_FUNCTIONS, SECTION_LEX, SECTION_MACRO_TYPES,
    SECTION_PAREN_PAIRS, SECTION_PREPROC_MASK, SECTION_PROGRAM_GRAPH,
    SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES, SECTION_SEMANTIC_PROGRAM_GRAPH_NODES, SECTION_SEMA_SCOPE,
    SECTION_VAST, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::{TOK_ARROW, TOK_DOT, TOK_ELLIPSIS, TOK_IF, TOK_RETURN};
use vyre_libs::parsing::c::parse::vast::{
    C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_COMPOUND_LITERAL_EXPR, C_AST_KIND_FIELD_DECL,
    C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_IF_STMT, C_AST_KIND_INITIALIZER_LIST,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_RANGE_DESIGNATOR_EXPR,
    C_AST_KIND_RETURN_STMT, C_AST_KIND_STRUCT_DECL,
};
use vyre_primitives::predicate::node_kind;

fn gpu_object_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("GPU object contract test mutex poisoned")
}

/// Kernel-style operations table, compound literal, designated initializers, statement expression,
/// GNU range designators, and `__attribute__((unused))` on an aggregate.
const LINUX_DRIVER_SHAPED: &str = r#"
typedef unsigned long ulong_t;

struct file_operations {
    int (*read)(void *f, void *buf, ulong_t len);
    void (*release)(void *f);
};

struct file {
    struct file_operations *f_op;
    int f_flags;
};

static int demo_read(void *f, void *buf, ulong_t len);

static int demo_read(void *f, void *buf, ulong_t len)
{
    (void)f;
    (void)buf;
    (void)len;
    return 0;
}

static void demo_release(void *f)
{
    (void)f;
}

static struct file_operations demo_fops __attribute__((unused)) = {
    .read = demo_read,
    .release = demo_release,
};

static int ranges[4] = { [0 ... 1] = 9, [3] = 1 };

static int linux_fop_open(struct file *filp)
{
    struct file local = (struct file){
        .f_op = &demo_fops,
        .f_flags = 0,
    };
    int bump = ({
        int t = local.f_flags;
        t + 3;
    });
    if (filp && filp->f_op && filp->f_op->read)
        bump += filp->f_op->read(filp, 0, 0);
    bump += ranges[0];
    return bump;
}
"#;

fn assert_core_pipeline_sections(object: &CompiledObject) {
    for tag in [
        SECTION_LEX,
        SECTION_PAREN_PAIRS,
        SECTION_BRACE_PAIRS,
        SECTION_FUNCTIONS,
        SECTION_CALLS,
        SECTION_PREPROC_MASK,
        SECTION_MACRO_TYPES,
        SECTION_AST,
        SECTION_CFG,
        SECTION_VAST,
        SECTION_EXPRESSION_SHAPE,
        SECTION_PROGRAM_GRAPH,
        SECTION_SEMANTIC_PROGRAM_GRAPH_NODES,
        SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES,
        SECTION_SEMA_SCOPE,
    ] {
        assert!(
            !object.section(tag).is_empty(),
            "VYRECOB2 section {tag} is non-empty"
        );
    }
}

fn assert_lex_spans_resident(object: &CompiledObject, resident: &str) {
    let lex = object.lex();
    assert!(
        !lex.tok_types.is_empty(),
        "non-empty translation unit produces tokens"
    );
    assert!(
        lex.tok_types
            .iter()
            .zip(&lex.starts)
            .zip(&lex.lens)
            .all(|((_, &start), &len)| {
                let start = start as usize;
                let end = start.saturating_add(len as usize);
                len > 0 && end <= resident.len()
            }),
        "lex section spans stay inside the resident translation unit"
    );
}

fn assert_macro_snapshot_matches_lex(object: &CompiledObject) {
    let lex = object.lex();
    assert_eq!(
        object.words(SECTION_MACRO_TYPES),
        lex.tok_types,
        "macro-types snapshot matches post-keyword token kinds for this pipeline revision"
    );
}

#[test]
fn linux_driver_shaped_tu_reaches_all_pipeline_sections_with_valid_lex_spans() {
    let _guard = gpu_object_guard();
    let (object, resident) = compile_source_with_resident(
        "linux_driver_shaped",
        LINUX_DRIVER_SHAPED,
        Vec::new(),
        Vec::new(),
    );
    object.assert_elf();
    assert_eq!(object.version(), 7);

    assert_core_pipeline_sections(&object);
    assert_lex_spans_resident(&object, &resident);
    assert_macro_snapshot_matches_lex(&object);

    let mask = object.words(SECTION_PREPROC_MASK);
    let lex = object.lex();
    assert_eq!(mask.len(), lex.tok_types.len());
    assert!(
        mask.iter().all(|&word| word == 1),
        "baseline conditional mask marks every token active for this TU"
    );
}

#[test]
fn linux_driver_shaped_tu_preserves_vast_pg_gnu_initializers_and_designators() {
    let _guard = gpu_object_guard();
    let (object, resident) = compile_source_with_resident(
        "linux_driver_vast_pg",
        LINUX_DRIVER_SHAPED,
        Vec::new(),
        Vec::new(),
    );
    object.assert_elf();

    let lex = object.lex();
    let vast_words = object.words(SECTION_VAST);
    let pg_words = object.words(SECTION_PROGRAM_GRAPH);
    assert_typed_vast_and_pg_rows(
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        &vast_words,
        &pg_words,
    );

    for (label, kind) in [
        ("struct decl", C_AST_KIND_STRUCT_DECL),
        ("field decl", C_AST_KIND_FIELD_DECL),
        ("pointer decl", C_AST_KIND_POINTER_DECL),
        ("function declarator", C_AST_KIND_FUNCTION_DECLARATOR),
        ("function definition", C_AST_KIND_FUNCTION_DEFINITION),
        ("GNU attribute", C_AST_KIND_GNU_ATTRIBUTE),
        ("unused attribute", C_AST_KIND_ATTRIBUTE_UNUSED),
        ("initializer list", C_AST_KIND_INITIALIZER_LIST),
        ("compound literal", C_AST_KIND_COMPOUND_LITERAL_EXPR),
        ("statement expression", C_AST_KIND_GNU_STATEMENT_EXPR),
        ("range designator", C_AST_KIND_RANGE_DESIGNATOR_EXPR),
    ] {
        assert!(
            vast_words
                .chunks_exact(VAST_STRIDE_U32)
                .any(|row| row[0] == kind),
            "VAST carries {label} (kind {kind})"
        );
        assert!(
            pg_words
                .chunks_exact(PG_STRIDE_U32)
                .any(|row| row[0] == kind),
            "ProgramGraph carries {label} (kind {kind})"
        );
    }

    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[0] == node_kind::FUNCTION_DECL),
        "VAST carries FUNCTION_DECL predicate nodes"
    );
    assert!(
        pg_words
            .chunks_exact(PG_STRIDE_U32)
            .any(|row| row[0] == node_kind::FUNCTION_DECL),
        "ProgramGraph carries FUNCTION_DECL predicate nodes"
    );

    for kind in [
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
            "VAST carries typed node kind {kind}"
        );
        assert!(
            pg_words
                .chunks_exact(PG_STRIDE_U32)
                .any(|row| row[0] == kind),
            "ProgramGraph carries typed node kind {kind}"
        );
    }

    let dot_designator = find_token_in_context(
        &resident,
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        TOK_DOT,
        "    .read = demo_read",
        ".",
    );
    assert_eq!(
        vast_kind(&vast_words, dot_designator),
        C_AST_KIND_MEMBER_ACCESS_EXPR,
        "designator `.read` lowers to member-access shaped VAST"
    );

    let arrow_f_op = find_token_in_context(
        &resident,
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        TOK_ARROW,
        "filp->f_op",
        "->",
    );
    assert_eq!(
        vast_kind(&vast_words, arrow_f_op),
        C_AST_KIND_MEMBER_ACCESS_EXPR,
        "`filp->f_op` arrow is member access"
    );

    let if_idx = find_kind(&lex.tok_types, TOK_IF);
    assert_eq!(vast_kind(&vast_words, if_idx), C_AST_KIND_IF_STMT);

    let ellipsis_idx = find_kind(&lex.tok_types, TOK_ELLIPSIS);
    assert_eq!(
        vast_kind(&vast_words, ellipsis_idx),
        C_AST_KIND_RANGE_DESIGNATOR_EXPR,
        "GNU range designator `...` token is classified in VAST"
    );

    let ret_open = find_token_in_context(
        &resident,
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        TOK_RETURN,
        "    return bump",
        "return",
    );
    assert_eq!(
        vast_kind(&vast_words, ret_open),
        C_AST_KIND_RETURN_STMT,
        "final return in linux_fop_open is a return statement"
    );

    let assign_inner = find_token_in_context(
        &resident,
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        TOK_DOT,
        "local.f_flags",
        ".",
    );
    assert_eq!(
        vast_kind(&vast_words, assign_inner),
        C_AST_KIND_MEMBER_ACCESS_EXPR,
        "`local.f_flags` inside the statement expression is member access"
    );
}

#[test]
fn linux_driver_shaped_tu_emits_semantic_scope_declarations_with_resident_spans() {
    let _guard = gpu_object_guard();
    let (object, resident) = compile_source_with_resident(
        "linux_driver_semantic_scope",
        LINUX_DRIVER_SHAPED,
        Vec::new(),
        Vec::new(),
    );
    object.assert_elf();

    let scope = decode_object_sema_scope(object.payload())
        .expect("Linux-shaped TU semantic scope section must decode");
    let symbols = scope.symbols().collect::<Vec<_>>();
    assert!(
        symbols.len() >= 8,
        "Linux-shaped TU must emit several semantic declaration rows, got {symbols:#?}"
    );
    for required_kind in ["typedef", "function_decl", "function", "variable"] {
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.decl_kind_name == required_kind),
            "semantic scope must contain a {required_kind} declaration row: {symbols:#?}"
        );
    }
    for symbol in symbols {
        let start = symbol.token_start as usize;
        let end = start.saturating_add(symbol.token_len as usize);
        assert!(
            symbol.token_len > 0 && end <= resident.len(),
            "semantic symbol span must stay inside resident TU: {symbol:#?}"
        );
        let spelling = &resident[start..end];
        assert!(
            spelling
                .bytes()
                .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric()),
            "semantic symbol span should point at an identifier spelling, got {spelling:?} for {symbol:#?}"
        );
    }
}

#[test]
fn resident_stream_inlines_include_and_expands_header_macros_on_gpu() {
    let _guard = gpu_object_guard();
    let tmp = std::env::temp_dir().join(format!(
        "vyre_frontend_c_linux_inc_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(
        tmp.join("ktypes.h"),
        r#"#define KVER_MAJOR 6
typedef int kerr_t;
struct kref { int count; };
"#,
    )
    .unwrap();

    let tu = r#"
#include "ktypes.h"
static kerr_t check_version(void)
{
    return (kerr_t)KVER_MAJOR;
}
"#;

    let (object, resident) =
        compile_source_with_resident("linux_include_tu", tu, Vec::new(), vec![tmp.clone()]);
    object.assert_elf();

    assert!(
        !resident.contains("#include"),
        "resident TU should inline quoted includes"
    );
    assert!(
        !resident.contains("#define KVER_MAJOR"),
        "header macro directives must not leak after GPU preprocessing: {resident:?}"
    );
    assert!(
        resident.contains("return ( kerr_t ) 6"),
        "header macro use should be expanded by GPU preprocessing: {resident:?}"
    );

    assert_core_pipeline_sections(&object);
    assert_lex_spans_resident(&object, &resident);
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn cli_define_state_expands_without_host_define_prefix() {
    let _guard = gpu_object_guard();
    let tu = "static int magic(void) { return MAGIC; }\n";
    let (object, resident) = compile_source_with_resident(
        "cli_magic_define",
        tu,
        vec![("MAGIC".to_string(), Some("0x42".to_string()))],
        Vec::new(),
    );
    object.assert_elf();
    assert!(
        !resident.contains("#define MAGIC"),
        "CLI -D must be carried as GPU macro state, not host-prepended source text: {resident:?}"
    );
    assert!(
        resident.contains("return 0x42"),
        "CLI -D macro invocation must be expanded by the GPU preprocessor, not left for a host rewrite path: {resident:?}"
    );
    assert_lex_spans_resident(&object, &resident);
}
