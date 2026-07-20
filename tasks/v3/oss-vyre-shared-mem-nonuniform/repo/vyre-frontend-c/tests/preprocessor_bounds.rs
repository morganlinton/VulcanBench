//! Preprocessor expansion bound and recursion contracts.

use vyre_frontend_c::tu_host::reference_expand_preprocessor_macros;

#[test]
fn mutual_recursion_macros_terminate_without_panic() {
    let source = r#"
#define A B
#define B A
int x = A;
"#;
    let out = reference_expand_preprocessor_macros(source);
    // The exact final token depends on depth parity, but it must not panic.
    assert!(
        out.contains("int x ="),
        "mutual recursion should terminate without panic, got:\n{out}"
    );
}

#[test]
fn deep_linear_macro_chain_terminates_gracefully() {
    let mut source = String::new();
    for i in 0..40 {
        source.push_str(&format!("#define M{} M{}\n", i, i + 1));
    }
    source.push_str("#define M40 final\n");
    source.push_str("int x = M0;\n");
    let out = reference_expand_preprocessor_macros(&source);
    // Depth limit is 32; chain of 40 exceeds it, but expansion must still terminate.
    assert!(
        out.contains("int x ="),
        "deep linear chain should terminate gracefully, got:\n{out}"
    );
}

#[test]
fn recursive_function_macro_terminates() {
    let source = r#"
#define REC(x) REC(x)
int x = REC(1);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int x ="),
        "recursive function macro should terminate, got:\n{out}"
    );
}

#[test]
fn self_referential_macro_expands_once() {
    let source = r#"
#define FOO FOO + 1
int x = FOO;
"#;
    let out = reference_expand_preprocessor_macros(source);
    // Standard C: self-referential macro is not infinitely expanded.
    // The intended contract is exactly one expansion: "FOO + 1".
    let line = out.lines().find(|l| l.contains("int x =")).unwrap_or("");
    assert!(
        line.trim() == "int x = FOO + 1;",
        "self-referential macro should expand exactly once then stop, got line:\n{line}\nfull output:\n{out}"
    );
}

#[test]
fn macro_expansion_depth_32_allows_33_levels() {
    let mut source = String::new();
    // 33 expansions: M0->M1 (depth 0) ... M32->final (depth 32). Depth 33 is the cutoff.
    for i in 0..32 {
        source.push_str(&format!("#define M{} M{}\n", i, i + 1));
    }
    source.push_str("#define M32 final\n");
    source.push_str("int x = M0;\n");
    let out = reference_expand_preprocessor_macros(&source);
    assert!(
        out.contains("int x = final;"),
        "chain of 33 expansions should fully expand, got:\n{out}"
    );
}

#[test]
fn macro_expansion_depth_34_stops_before_end() {
    let mut source = String::new();
    // 34 expansions: M0->M1 ... M33->final. Depth 33 blocks the last step.
    for i in 0..33 {
        source.push_str(&format!("#define M{} M{}\n", i, i + 1));
    }
    source.push_str("#define M33 final\n");
    source.push_str("int x = M0;\n");
    let out = reference_expand_preprocessor_macros(&source);
    assert!(
        !out.contains("int x = final;"),
        "chain of 34 expansions should hit depth limit before final, got:\n{out}"
    );
    assert!(
        out.contains("int x ="),
        "expansion should still produce a valid line, got:\n{out}"
    );
}
