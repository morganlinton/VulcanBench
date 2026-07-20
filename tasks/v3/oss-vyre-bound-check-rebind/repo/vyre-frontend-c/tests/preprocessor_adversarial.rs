//! Preprocessor macro expansion + conditional evaluation adversarial tests.
//!
//! The vyre-frontend-c preprocessor must handle C11 #define/#ifdef/#if/#elif/#else/#endif
//! with correct scoping, nesting, variadic macros, token paste (##), and
//! stringification (#). These tests exercise boundary conditions that have
//! historically caused mis-expansion in production C preprocessors.

use vyre_frontend_c::tu_host::reference_expand_preprocessor_macros;

// ── Object-like macros ───────────────────────────────────────────────

#[test]
fn simple_object_macro_expansion() {
    let src = "#define FOO 42\nint x = FOO;\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int x = 42;"),
        "Fix: simple object macro must expand, got: {out}"
    );
}

#[test]
fn chained_object_macro_expansion() {
    let src = "#define A 1\n#define B A\nint x = B;\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int x = 1;"),
        "Fix: chained macros must recursively expand, got: {out}"
    );
}

#[test]
fn undef_removes_macro() {
    let src = "#define FOO 42\n#undef FOO\nint x = FOO;\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int x = FOO;"),
        "Fix: #undef must remove macro so it expands literally, got: {out}"
    );
}

#[test]
fn macro_does_not_expand_in_string_literal() {
    let src = "#define FOO bar\nchar *s = \"FOO\";\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("\"FOO\""),
        "Fix: macros must not expand inside string literals, got: {out}"
    );
}

// ── Function-like macros ─────────────────────────────────────────────

#[test]
fn function_macro_with_args() {
    let src = "#define ADD(a, b) ((a) + (b))\nint x = ADD(1, 2);\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("((1) + (2))"),
        "Fix: function macro must substitute args, got: {out}"
    );
}

#[test]
fn function_macro_nested_parens_in_args() {
    let src = "#define F(x) x\nint y = F((1 + 2));\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("(1 + 2)"),
        "Fix: nested parens in macro args must be preserved, got: {out}"
    );
}

#[test]
fn variadic_macro_va_args() {
    let src = "#define LOG(fmt, ...) printf(fmt, __VA_ARGS__)\nLOG(\"x=%d y=%d\", 1, 2);\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("printf(\"x=%d y=%d\", 1, 2)"),
        "Fix: __VA_ARGS__ must expand to trailing args, got: {out}"
    );
}

// ── Token paste (##) ─────────────────────────────────────────────────

#[test]
fn token_paste_concatenates() {
    let src = "#define PASTE(a, b) a ## b\nint PASTE(foo, bar);\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int foobar;"),
        "Fix: ## must concatenate tokens, got: {out}"
    );
}

// ── Stringification (#) ─────────────────────────────────────────────

#[test]
fn stringify_operator() {
    let src = "#define STR(x) #x\nchar *s = STR(hello);\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("\"hello\""),
        "Fix: # must stringify the argument, got: {out}"
    );
}

// ── Conditionals ─────────────────────────────────────────────────────

#[test]
fn ifdef_includes_when_defined() {
    let src = "#define X\n#ifdef X\nyes\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("yes"),
        "Fix: #ifdef must include when macro is defined, got: {out}"
    );
}

#[test]
fn ifdef_excludes_when_undefined() {
    let src = "#ifdef UNDEFINED_MACRO\nno\n#endif\nyes\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        !out.contains("no"),
        "Fix: #ifdef must exclude when macro is undefined, got: {out}"
    );
    assert!(out.contains("yes"));
}

#[test]
fn ifndef_includes_when_undefined() {
    let src = "#ifndef NOPE\nyes\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("yes"),
        "Fix: #ifndef must include when macro is undefined, got: {out}"
    );
}

#[test]
fn ifdef_else_branch() {
    let src = "#ifdef NOPE\nno\n#else\nyes\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        !out.contains("no"),
        "Fix: #ifdef false path must be excluded."
    );
    assert!(out.contains("yes"), "Fix: #else branch must be included.");
}

#[test]
fn nested_conditionals() {
    let src = "#define A\n#ifdef A\n#ifdef B\nno\n#else\nyes\n#endif\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(!out.contains("no"));
    assert!(
        out.contains("yes"),
        "Fix: nested conditionals must evaluate correctly, got: {out}"
    );
}

#[test]
fn if_with_numeric_expr() {
    let src = "#if 1 + 1 == 2\nyes\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("yes"),
        "Fix: #if with numeric expression must evaluate, got: {out}"
    );
}

#[test]
fn elif_chain() {
    let src = "#if 0\nno1\n#elif 0\nno2\n#elif 1\nyes\n#else\nno3\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(!out.contains("no1"));
    assert!(!out.contains("no2"));
    assert!(!out.contains("no3"));
    assert!(
        out.contains("yes"),
        "Fix: #elif chain must select the correct branch, got: {out}"
    );
}

#[test]
fn deeply_nested_conditionals() {
    let src = "\
#define A 1
#if A
  #if 0
    no
  #else
    #if A
      yes
    #endif
  #endif
#endif
";
    let out = reference_expand_preprocessor_macros(src);
    assert!(!out.contains("no"));
    assert!(
        out.contains("yes"),
        "Fix: deeply nested conditionals must resolve, got: {out}"
    );
}

// ── Self-referential macro guard ─────────────────────────────────────

#[test]
fn self_referential_macro_does_not_recurse_infinitely() {
    let src = "#define X X\nint x = X;\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int x = X;"),
        "Fix: self-referential macro must not recurse infinitely, got: {out}"
    );
}

// ── Empty and degenerate inputs ──────────────────────────────────────

#[test]
fn empty_input_produces_empty_output() {
    let out = reference_expand_preprocessor_macros("");
    assert!(out.is_empty() || out.trim().is_empty());
}

#[test]
fn no_directives_passes_through() {
    let src = "int main() { return 0; }\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int main()"),
        "Fix: source without directives must pass through unchanged."
    );
}

#[test]
fn define_with_no_value_is_empty_string() {
    let src = "#define EMPTY\n#ifdef EMPTY\nyes\n#endif\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("yes"),
        "Fix: #define with no value must still be defined, got: {out}"
    );
}

// ── Comment stripping in directives ──────────────────────────────────

#[test]
fn line_comment_in_define_is_stripped() {
    let src = "#define X 42 // this is a comment\nint x = X;\n";
    let out = reference_expand_preprocessor_macros(src);
    assert!(
        out.contains("int x = 42"),
        "Fix: line comments in #define must be stripped, got: {out}"
    );
    assert!(
        !out.contains("// this"),
        "Fix: comment text must not appear in expansion, got: {out}"
    );
}
