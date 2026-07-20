//! GPU frontend coverage for Linux-grade GNU/C11 constructs that already have
//! reference-oracle contracts in `vyre-libs`.

mod support;

use support::*;

const LINUX_GRADE_CONSTRUCTS: &str = r#"
typedef unsigned long size_t;

static void cleanup_int(int *p) { *p = 0; }
__attribute__((constructor)) static void init_ctor(void) { }
__attribute__((destructor)) static void fini_dtor(void) { }

struct __attribute__((packed)) packed_holder {
    char tag;
    int value;
};

__attribute__((aligned(8))) static long aligned_value;
typedef unsigned int __attribute__((mode(__word__))) word_mode_t;

int linux_grade_constructs(int n)
{
    __auto_type local = n;
    typeof(local) copy = local;
    __typeof__(copy) other = copy;
    __attribute__((cleanup(cleanup_int))) int scoped = other;
    int expr_value = ({ int tmp = scoped + 1; tmp; });
    void *target = &&done;

    if (expr_value < 0)
        goto *target;

    _Static_assert(sizeof(int) >= 4, "int size");
    _Alignas(16) char buf[16];
    aligned_value = _Alignof(long) + sizeof(buf);

done:
    return expr_value + (int)aligned_value;
}
"#;

#[test]
fn linux_grade_gnu_c11_constructs_compile_on_gpu_frontend() {
    let (object, _resident) = compile_source_with_resident(
        "linux_grade_constructs_gpu",
        LINUX_GRADE_CONSTRUCTS,
        Vec::new(),
        Vec::new(),
    );
    object.assert_elf();

    assert_ne!(object.section(SECTION_LEX).len(), 0, "lex section present");
    assert_ne!(
        object.section(SECTION_VAST).len(),
        0,
        "VAST section present"
    );
    assert_ne!(
        object.section(SECTION_PROGRAM_GRAPH).len(),
        0,
        "program graph section present"
    );
    assert_ne!(
        object.section(SECTION_SEMA_SCOPE).len(),
        0,
        "semantic scope section present"
    );
    assert_ne!(
        object.section(SECTION_EXPRESSION_SHAPE).len(),
        0,
        "expression-shape section present"
    );
}
