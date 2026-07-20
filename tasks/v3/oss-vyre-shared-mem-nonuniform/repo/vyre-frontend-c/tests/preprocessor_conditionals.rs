//! Preprocessor conditional nesting contracts.

use vyre_frontend_c::tu_host::reference_expand_preprocessor_macros;

#[test]
fn nested_ifdef_inside_active_if_branch() {
    let source = r#"
#define OUTER 1
#define INNER 1
#if OUTER
#ifdef INNER
int x = 1;
#else
int x = 2;
#endif
#else
int x = 3;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int x = 1;"),
        "expected inner active, got:\n{out}"
    );
    assert!(!out.contains("int x = 2;"));
    assert!(!out.contains("int x = 3;"));
}

#[test]
fn nested_ifdef_inside_inactive_if_branch_is_skipped() {
    let source = r#"
#define OUTER 0
#define INNER 1
#if OUTER
#ifdef INNER
int x = 1;
#else
int x = 2;
#endif
#else
int x = 3;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(!out.contains("int x = 1;"));
    assert!(!out.contains("int x = 2;"));
    assert!(
        out.contains("int x = 3;"),
        "expected outer else, got:\n{out}"
    );
}

#[test]
fn deeply_nested_mixed_conditionals() {
    let source = r#"
#define A 1
#define B 0
#define C 1
#if A
#ifndef B
int l1 = 1;
#elif C
int l1 = 2;
#else
int l1 = 3;
#endif
#if B
int l2 = 1;
#elif C
int l2 = 2;
#else
int l2 = 3;
#endif
#else
int l1 = 4;
int l2 = 4;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    // B IS defined (as 0), so #ifndef B is false; #elif C is true.
    assert!(
        out.contains("int l1 = 2;"),
        "B is defined so #ifndef B is false, #elif C is true, got:\n{out}"
    );
    assert!(
        out.contains("int l2 = 2;"),
        "B is 0 so first #if B is false, #elif C is true, got:\n{out}"
    );
    assert!(!out.contains("int l1 = 4;"));
    assert!(!out.contains("int l2 = 4;"));
}

#[test]
fn five_level_nesting_all_true() {
    let source = r#"
#define L1 1
#define L2 1
#define L3 1
#define L4 1
#define L5 1
#if L1
#if L2
#if L3
#if L4
#if L5
int deep = 1;
#endif
#endif
#endif
#endif
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int deep = 1;"),
        "expected 5-level nesting to work, got:\n{out}"
    );
}

#[test]
fn five_level_nesting_middle_false() {
    let source = r#"
#define L1 1
#define L2 1
#define L3 0
#define L4 1
#define L5 1
#if L1
#if L2
#if L3
int deep = 1;
#else
int deep = 2;
#endif
#endif
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(!out.contains("int deep = 1;"));
    assert!(
        out.contains("int deep = 2;"),
        "expected middle false branch, got:\n{out}"
    );
}

#[test]
fn elif_evaluated_in_order() {
    let source = r#"
#define VAL 20
#if VAL == 10
int branch = 10;
#elif VAL == 20
int branch = 20;
#elif VAL == 30
int branch = 30;
#else
int branch = 99;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int branch = 20;"),
        "expected second elif, got:\n{out}"
    );
    assert!(!out.contains("int branch = 10;"));
    assert!(!out.contains("int branch = 30;"));
    assert!(!out.contains("int branch = 99;"));
}

#[test]
fn elif_skips_remaining_after_first_true() {
    let source = r#"
#define VAL 5
#if VAL == 5
int first = 1;
#elif VAL == 5
int second = 1;
#else
int third = 1;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("int first = 1;"));
    assert!(
        !out.contains("int second = 1;"),
        "elif after true branch must be skipped"
    );
    assert!(
        !out.contains("int third = 1;"),
        "else after true branch must be skipped"
    );
}

#[test]
fn else_runs_when_all_elif_false() {
    let source = r#"
#define VAL 99
#if VAL == 1
int branch = 1;
#elif VAL == 2
int branch = 2;
#else
int branch = 99;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(!out.contains("int branch = 1;"));
    assert!(!out.contains("int branch = 2;"));
    assert!(
        out.contains("int branch = 99;"),
        "expected else branch, got:\n{out}"
    );
}

#[test]
fn nested_elif_inside_outer_active_branch() {
    let source = r#"
#define OUTER 1
#define INNER 2
#if OUTER
#if INNER == 1
int x = 1;
#elif INNER == 2
int x = 2;
#else
int x = 3;
#endif
#else
int x = 4;
#endif
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int x = 2;"),
        "expected nested elif, got:\n{out}"
    );
    assert!(!out.contains("int x = 1;"));
    assert!(!out.contains("int x = 3;"));
    assert!(!out.contains("int x = 4;"));
}

#[test]
fn directives_inside_inactive_branch_do_not_affect_macro_state() {
    let source = r#"
#define FOO 1
#if 0
#define FOO 2
#undef FOO
#endif
int x = FOO;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int x = 1;"),
        "inactive branch must not affect macros, got:\n{out}"
    );
}

#[test]
fn undef_of_undefined_macro_is_silent() {
    let source = r#"
#undef NEVER_DEFINED
int x = 1;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int x = 1;"),
        "#undef of undefined macro should be silent, got:\n{out}"
    );
}

#[test]
fn undef_then_redefine_uses_new_value() {
    let source = r#"
#define VAL 1
int a = VAL;
#undef VAL
#define VAL 2
int b = VAL;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("int a = 1;"));
    assert!(out.contains("int b = 2;"));
}

#[test]
fn undef_inside_conditional_block_only_when_active() {
    let source = r#"
#define FOO 1
#if 0
#undef FOO
#endif
int a = FOO;
#if 1
#undef FOO
#endif
int b = FOO;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = 1;"),
        "inactive block should not undef, got:\n{out}"
    );
    assert!(
        out.contains("int b = FOO;"),
        "active block should undef, got:\n{out}"
    );
}
