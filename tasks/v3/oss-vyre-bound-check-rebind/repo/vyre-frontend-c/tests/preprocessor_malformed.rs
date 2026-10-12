//! Malformed and edge-case preprocessor macro invocation contracts.

use vyre_frontend_c::tu_host::reference_expand_preprocessor_macros;

#[test]
fn function_macro_without_parens_remains_unexpanded() {
    let source = r#"
#define FOO(x) ((x) + 1)
int a = FOO;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = FOO;"),
        "function-like macro without parens should not expand, got:\n{out}"
    );
}

#[test]
fn function_macro_unclosed_paren_remains_unexpanded() {
    let source = r#"
#define FOO(x) ((x) + 1)
int a = FOO(1;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = FOO(1;"),
        "unclosed paren should prevent expansion, got:\n{out}"
    );
}

#[test]
fn function_macro_too_few_args_substitutes_empty() {
    let source = r#"
#define FOO(a, b) ((a) + (b))
int a = FOO(1);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = ((1) + ());"),
        "missing argument should expand to empty tokens, got:\n{out}"
    );
}

#[test]
fn function_macro_extra_args_are_ignored() {
    let source = r#"
#define FOO(a) ((a) + 1)
int a = FOO(1, 2, 3);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = ((1) + 1);"),
        "extra arguments should be ignored, got:\n{out}"
    );
}

#[test]
fn object_macro_with_following_parens_expands_to_value() {
    let source = r#"
#define FOO 42
int a = FOO(1);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = 42(1);"),
        "object macro should expand regardless of following tokens, got:\n{out}"
    );
}

#[test]
fn malformed_define_with_invalid_name_is_ignored() {
    let source = r#"
#define 123invalid 1
int a = 123invalid;
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = 123invalid;"),
        "malformed #define should be ignored, got:\n{out}"
    );
}

#[test]
fn function_macro_with_only_whitespace_between_name_and_paren() {
    let source = r#"
#define FOO(x) ((x) * 2)
int a = FOO (1);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = ((1) * 2);"),
        "whitespace between macro name and paren is still a function-like invocation, got:\n{out}"
    );
}

#[test]
fn nested_parens_in_macro_args_parsed_correctly() {
    let source = r#"
#define FOO(x) ((x) + 1)
int a = FOO((2 + 3) * 4);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains("int a = (((2 + 3) * 4) + 1);"),
        "nested parens in arguments should be parsed correctly, got:\n{out}"
    );
}

#[test]
fn comma_inside_string_in_macro_arg_not_split() {
    let source = r#"
#define FOO(a, b) a b
int a = FOO("hello, world", 1);
"#;
    let out = reference_expand_preprocessor_macros(source);
    assert!(
        out.contains(r#"int a = "hello, world" 1;"#),
        "comma inside string literal should not split args, got:\n{out}"
    );
}
