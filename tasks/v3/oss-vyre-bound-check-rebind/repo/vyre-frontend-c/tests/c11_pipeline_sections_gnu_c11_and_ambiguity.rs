//! GNU C11 ambiguity cases: object pipeline still emits complete section tables.
mod support;

use support::*;
use vyre_libs::parsing::c::lex::tokens::TOK_IDENTIFIER;

const SOURCE_GNU_C11_AMBIGUITY: &str = r#"
typedef unsigned long size_t;

struct S {
    int field;
    struct S *next;
};

_Atomic int atomic_counter;

static size_t builtin_suite(struct S *p)
{
    size_t o = __builtin_offsetof(struct S, field);
    size_t s = __builtin_object_size(p, 0);
    __builtin_prefetch(p, 0, 3);

    switch (o) {
    case 0:
        return s;
    case 1 ... 5:
        goto done;
    default:
        break;
    }

    for (int i = 0; i < (int)o; i++) {
        if (p && p->next)
            p = p->next;
    }

done:
    __builtin_unreachable();
    return 0;
}

static void *label_addr(void)
{
    void *addr = &&target;
target:
    return addr;
}
"#;

#[test]
fn compile_gnu_c11_and_ambiguity_reaches_all_pipeline_sections() {
    let object = compile_source(
        "gnu_c11_and_ambiguity",
        SOURCE_GNU_C11_AMBIGUITY,
        Vec::new(),
    );
    object.assert_elf();

    let lex = object.lex();
    assert!(
        !lex.tok_types.is_empty(),
        "real translation unit with GNU/C11/ambiguity constructs produced a lexed token stream"
    );
    assert!(
        lex.tok_types
            .iter()
            .zip(&lex.starts)
            .zip(&lex.lens)
            .all(|((_, &start), &len)| {
                let start = start as usize;
                let end = start.saturating_add(len as usize);
                len > 0 && end <= SOURCE_GNU_C11_AMBIGUITY.len()
            }),
        "lex section spans stay inside the prepared translation unit"
    );

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
        SECTION_SEMA_SCOPE,
    ] {
        assert!(
            !object.section(tag).is_empty(),
            "VYRECOB2 section {tag} is non-empty for GNU/C11/ambiguity source"
        );
    }

    let sema_words = object.words(SECTION_SEMA_SCOPE);
    assert_eq!(sema_words.len(), lex.tok_types.len() * SEMA_STRIDE_U32);
    for (idx, &token_type) in lex.tok_types.iter().enumerate() {
        let intern_id = sema_words[idx * SEMA_STRIDE_U32 + 3];
        if token_type == TOK_IDENTIFIER {
            assert_ne!(
                intern_id, 0,
                "semantic scope pass interns emitted identifier token {idx}"
            );
        }
    }

    assert_eq!(object.words(SECTION_PAREN_PAIRS).len(), lex.tok_types.len());
    assert_eq!(object.words(SECTION_BRACE_PAIRS).len(), lex.tok_types.len());
}
