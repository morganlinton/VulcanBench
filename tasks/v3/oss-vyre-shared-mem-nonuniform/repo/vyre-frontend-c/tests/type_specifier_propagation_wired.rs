//! Type-specifier propagation stage wiring smoke test.

mod support;

use support::compile_source_with_resident;

const SOURCE: &str = r#"
unsigned long long int x = 42;
const volatile int *p;
"#;

#[test]
fn multi_word_type_specifiers_compile_without_panic() {
    let (object, _resident) =
        compile_source_with_resident("type_spec_prop", SOURCE, Vec::new(), Vec::new());
    object.assert_elf();
}
