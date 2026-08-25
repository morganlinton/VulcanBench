//! VYRECOB2 version stability contract: every translation unit shape emits version 7.

mod support;

use support::{compile_source, ObjectEnvelope, ObjectFlavor};

fn assert_stable_version(name: &str, object: support::CompiledObject) {
    object.assert_elf();
    let env = ObjectEnvelope::from_elf(object.into_inner());
    env.assert_carrier();
    env.assert_version(7);
    assert_eq!(
        env.flavor(),
        ObjectFlavor::Elf,
        "{name}: pipeline emits ELF carrier"
    );
    assert!(
        env.section_count() > 0,
        "{name}: stable version object carries at least one section"
    );
}

#[test]
fn minimal_tu_emits_stable_vyrecob2_version() {
    let object = compile_source("version_minimal", "int x;\n", Vec::new());
    assert_stable_version("minimal", object);
}

#[test]
fn gnu_c11_tu_emits_stable_vyrecob2_version() {
    let source = r#"
typedef unsigned long size_t;
static int __attribute__((unused)) x = 1;
static size_t f(void) { return 0; }
"#;
    let object = compile_source("version_gnu", source, Vec::new());
    assert_stable_version("gnu_c11", object);
}

#[test]
fn linux_shaped_tu_emits_stable_vyrecob2_version() {
    let source = r#"
struct file_operations {
    int (*read)(void *f, void *buf, unsigned long len);
};
static struct file_operations ops = { .read = 0 };
"#;
    let object = compile_source("version_linux", source, Vec::new());
    assert_stable_version("linux_shaped", object);
}
