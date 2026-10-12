use super::*;

#[test]
fn ifdef_when_macro_defined_keeps_active_block() {
    let loader = MemLoader::new();
    let out = run(
        b"#ifdef FOO\nint a;\n#endif\nint b;",
        &[MacroDef {
            name: b"FOO".to_vec().into(),
            args: Vec::new(),
            body: Vec::new(),
            is_function_like: false,
        }],
        &loader,
    );
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("a"));
    assert!(s.contains("b"));
}

#[test]
fn ifdef_when_macro_undefined_drops_inactive_block() {
    let loader = MemLoader::new();
    let out = run(b"#ifdef MISSING\nint a;\n#endif\nint b;", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(!s.contains("a"), "inactive #ifdef block must NOT emit 'a'");
    assert!(s.contains("b"));
}

#[test]
fn ifndef_inverts() {
    let loader = MemLoader::new();
    let out = run(b"#ifndef MISSING\nint a;\n#endif\n", &[], &loader);
    assert!(String::from_utf8_lossy(&out).contains("a"));
}

#[test]
fn else_branch_taken_when_if_false() {
    let loader = MemLoader::new();
    let out = run(b"#if 0\nint a;\n#else\nint b;\n#endif\n", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(!s.contains("a"));
    assert!(s.contains("b"));
}

#[test]
fn elif_else_chain_picks_first_truthy() {
    let loader = MemLoader::new();
    let out = run(
        b"#if 0\nint a;\n#elif 1\nint b;\n#elif 1\nint c;\n#else\nint d;\n#endif\n",
        &[],
        &loader,
    );
    let s = String::from_utf8_lossy(&out);
    assert!(!s.contains("a"));
    assert!(s.contains("b"));
    assert!(!s.contains("c"));
    assert!(!s.contains("d"));
}

#[test]
fn if_divide_by_zero_fails_loudly() {
    let loader = MemLoader::new();
    let err = run_err(b"#if 4 / 0\nint hidden;\n#endif\n", &[], &loader);
    assert!(
        err.contains("malformed #if/#elif expression"),
        "divide-by-zero #if must fail before conditional masking; got {err}"
    );
}

#[test]
fn if_modulo_by_zero_fails_loudly() {
    let loader = MemLoader::new();
    let err = run_err(b"#if 4 % 0\nint hidden;\n#endif\n", &[], &loader);
    assert!(
        err.contains("malformed #if/#elif expression"),
        "modulo-by-zero #if must fail before conditional masking; got {err}"
    );
}

#[test]
fn nested_conditionals_inherit_parent_inactivity() {
    let loader = MemLoader::new();
    let out = run(
        b"#if 0\n#if 1\nint a;\n#endif\n#endif\nint b;\n",
        &[],
        &loader,
    );
    let s = String::from_utf8_lossy(&out);
    assert!(
        !s.contains("a"),
        "nested branch must inherit parent's inactivity"
    );
    assert!(s.contains("b"));
}
