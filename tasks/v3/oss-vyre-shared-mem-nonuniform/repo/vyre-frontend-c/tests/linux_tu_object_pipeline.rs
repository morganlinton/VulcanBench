//! Linux-shaped translation unit object pipeline contracts.

mod support;

use support::{
    compile_source_with_resident, ObjectEnvelope, SECTION_AST, SECTION_BRACE_PAIRS, SECTION_CALLS,
    SECTION_CFG, SECTION_EXPRESSION_SHAPE, SECTION_FUNCTIONS, SECTION_LEX, SECTION_MACRO_TYPES,
    SECTION_PAREN_PAIRS, SECTION_PREPROC_MASK, SECTION_PROGRAM_GRAPH,
    SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES, SECTION_SEMANTIC_PROGRAM_GRAPH_NODES, SECTION_SEMA_SCOPE,
    SECTION_VAST,
};

const LINUX_TU: &str = r#"
typedef unsigned long ulong_t;

struct file_operations {
    int (*read)(void *f, void *buf, ulong_t len);
    void (*release)(void *f);
};

struct file {
    struct file_operations *f_op;
    int f_flags;
};

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
    return bump;
}
"#;

#[test]
fn linux_shaped_tu_produces_elf_with_all_pipeline_sections() {
    let (object, resident) =
        compile_source_with_resident("linux_tu_pipeline", LINUX_TU, Vec::new(), Vec::new());
    object.assert_elf();

    let lex = object.lex();
    let env = ObjectEnvelope::from_elf(object.into_inner());
    env.assert_version(7);

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
        env.assert_section_present(tag);
        assert!(
            !env.section(tag).unwrap().is_empty(),
            "Linux-shaped TU section {tag} is non-empty"
        );
    }

    assert!(
        !lex.tok_types.is_empty(),
        "Linux TU produces a non-empty token stream"
    );
    assert!(
        lex.tok_types
            .iter()
            .zip(&lex.starts)
            .zip(&lex.lens)
            .all(|((_, &s), &l)| {
                let end = (s as usize).saturating_add(l as usize);
                l > 0 && end <= resident.len()
            }),
        "Linux TU lex spans stay inside resident source"
    );
}
