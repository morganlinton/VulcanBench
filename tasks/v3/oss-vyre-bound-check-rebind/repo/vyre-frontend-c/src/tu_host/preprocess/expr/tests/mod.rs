use super::*;

fn object_macro(value: &str) -> MacroDef {
    MacroDef {
        params: None,
        variadic: None,
        replacement: value.to_string(),
    }
}

fn function_macro(params: &[&str], value: &str) -> MacroDef {
    MacroDef {
        params: Some(params.iter().map(|param| (*param).to_string()).collect()),
        variadic: None,
        replacement: value.to_string(),
    }
}

fn variadic_function_macro(params: &[&str], variadic: &str, value: &str) -> MacroDef {
    MacroDef {
        params: Some(params.iter().map(|param| (*param).to_string()).collect()),
        variadic: Some(variadic.to_string()),
        replacement: value.to_string(),
    }
}

#[test]
fn object_like_macro_replacement_participates_in_if_expression() {
    let mut macros = HashMap::new();
    macros.insert("FEATURE".to_string(), object_macro("(1)"));
    assert!(eval_preproc_expr("FEATURE", &macros));
    assert!(eval_preproc_expr("FEATURE && 1", &macros));
}

#[test]
fn nested_object_like_macro_replacement_is_bounded_and_evaluated() {
    let mut macros = HashMap::new();
    macros.insert("A".to_string(), object_macro("B"));
    macros.insert("B".to_string(), object_macro("0x10UL"));
    assert!(eval_preproc_expr("A == 16", &macros));
}

#[test]
fn c_integer_literals_accept_common_radices_and_suffixes() {
    let macros = HashMap::new();
    assert!(eval_preproc_expr("0x10 == 16", &macros));
    assert!(eval_preproc_expr("010 == 8", &macros));
    assert!(eval_preproc_expr("0b1000 == 8", &macros));
    assert!(eval_preproc_expr("1ULL && 2L", &macros));
}

#[test]
fn character_literals_accept_common_c_escapes() {
    let macros = HashMap::new();
    assert!(eval_preproc_expr("'A' == 65", &macros));
    assert!(eval_preproc_expr("'\\n' == 10", &macros));
    assert!(eval_preproc_expr("'\\x41' == 'A'", &macros));
    assert!(eval_preproc_expr("'\\101' == 'A'", &macros));
    assert!(eval_preproc_expr("'AB' == 0x4142", &macros));
}

#[test]
#[should_panic(expected = "empty")]
fn empty_character_literal_fails_loudly() {
    let macros = HashMap::new();
    let _ = eval_preproc_expr("''", &macros);
}

#[test]
fn unsigned_max_literals_and_relational_guards_evaluate() {
    let mut macros = HashMap::new();
    macros.insert("MAXU".to_string(), object_macro("18446744073709551615UL"));
    assert!(eval_preproc_expr("MAXU > 9223372036854775807L", &macros));
    assert!(eval_preproc_expr("MAXU >= 18446744073709551615UL", &macros));
    assert!(eval_preproc_expr("16 <= 0x10", &macros));
}

#[test]
fn arithmetic_version_guards_and_bit_masks_evaluate() {
    let mut macros = HashMap::new();
    macros.insert("__GNUC__".to_string(), object_macro("4"));
    macros.insert("__GNUC_MINOR__".to_string(), object_macro("2"));
    assert!(eval_preproc_expr(
        "__GNUC__ * 100 + __GNUC_MINOR__ >= 402",
        &macros
    ));
    assert!(eval_preproc_expr("(1U << 5) == 32", &macros));
    assert!(eval_preproc_expr("(0x33 & 0x30) == 0x30", &macros));
    assert!(eval_preproc_expr("(0x10 | 0x2) == 0x12", &macros));
    assert!(eval_preproc_expr("(0x13 ^ 0x1) == 0x12", &macros));
    assert!(eval_preproc_expr("(~0U) != 0", &macros));
}

#[test]
fn function_like_macros_consume_arguments_in_if_expression() {
    let mut macros = HashMap::new();
    macros.insert("__has_attribute".to_string(), function_macro(&["x"], "0"));
    macros.insert("ZERO".to_string(), function_macro(&[], "1"));
    macros.insert("IDENTITY".to_string(), function_macro(&["x"], "x"));
    assert!(!eval_preproc_expr(
        "__has_attribute(always_inline)",
        &macros
    ));
    assert!(eval_preproc_expr("ZERO()", &macros));
    assert!(eval_preproc_expr("IDENTITY(4 + 5) == 9", &macros));
}

#[test]
fn unsupported_preprocessor_probe_builtins_consume_arguments_as_false() {
    let macros = HashMap::new();
    assert!(!eval_preproc_expr("__has_include(<stdio.h>)", &macros));
    assert!(!eval_preproc_expr(
        "__has_builtin(__builtin_expect)",
        &macros
    ));
    assert!(!eval_preproc_expr(
        "__has_feature(address_sanitizer)",
        &macros
    ));
    assert!(!eval_preproc_expr(
        "__has_embed(__FILE__ limit (4) vendor::attr(42))",
        &macros
    ));
    assert!(!eval_preproc_expr(
        "__has_warning(\"-Wunknown-warning\")",
        &macros
    ));
    assert!(eval_preproc_expr("!__has_c_attribute(nodiscard)", &macros));
    assert!(eval_preproc_expr(
        "!__has_cpp_attribute(clang::trivial_abi)",
        &macros
    ));
    assert!(eval_preproc_expr(
        "!__has_declspec_attribute(dllexport)",
        &macros
    ));
    assert!(eval_preproc_expr("__is_identifier(regular_name)", &macros));
    assert!(eval_preproc_expr("!__is_identifier(__int128)", &macros));
    assert!(eval_preproc_expr("!__is_identifier(typeof)", &macros));
}

#[test]
#[should_panic(expected = "received 2 arguments")]
fn function_like_macro_wrong_arity_fails_loudly() {
    let mut macros = HashMap::new();
    macros.insert("ONE".to_string(), function_macro(&["x"], "x"));
    let _ = eval_preproc_expr("ONE(1, 2)", &macros);
}

#[test]
fn variadic_function_like_macro_in_if_substitutes_variadic_arguments() {
    let mut macros = HashMap::new();
    macros.insert(
        "ANY".to_string(),
        variadic_function_macro(&["x"], "__VA_ARGS__", "x || __VA_ARGS__"),
    );
    macros.insert(
        "ANY_NAMED".to_string(),
        variadic_function_macro(&["x"], "rest", "x + rest"),
    );
    assert!(eval_preproc_expr("ANY(0, 1)", &macros));
    assert!(eval_preproc_expr("ANY_NAMED(2, 3) == 5", &macros));
}

#[test]
#[should_panic(expected = "malformed preprocessor defined operator")]
fn malformed_defined_without_identifier_fails_loudly() {
    let macros = HashMap::new();
    let _ = eval_preproc_expr("defined()", &macros);
}

#[test]
#[should_panic(expected = "missing `)`")]
fn malformed_defined_without_closing_paren_fails_loudly() {
    let macros = HashMap::new();
    let _ = eval_preproc_expr("defined(FEATURE", &macros);
}

#[test]
fn ternary_conditionals_select_constant_expression_branch() {
    let mut macros = HashMap::new();
    macros.insert("ENABLED".to_string(), object_macro("1"));
    assert!(eval_preproc_expr("ENABLED ? 7 : 0", &macros));
    assert!(eval_preproc_expr("0 ? 0 : 9", &macros));
    assert!(eval_preproc_expr("(ENABLED ? 4 : 2) == 4", &macros));
}

#[test]
fn logical_and_ternary_expressions_do_not_evaluate_inactive_division() {
    let macros = HashMap::new();
    assert!(!eval_preproc_expr("0 && (1 / 0)", &macros));
    assert!(eval_preproc_expr("1 || (1 / 0)", &macros));
    assert!(eval_preproc_expr("0 ? (1 / 0) : 1", &macros));
    assert!(eval_preproc_expr("1 ? 1 : (1 / 0)", &macros));
}

#[test]
#[should_panic(expected = "exceeding the i128 evaluator width")]
fn active_overwide_shift_fails_loudly() {
    let macros = HashMap::new();
    let _ = eval_preproc_expr("1 << 128", &macros);
}

#[test]
fn inactive_overwide_shift_is_not_evaluated() {
    let macros = HashMap::new();
    assert!(!eval_preproc_expr("0 && (1 << 128)", &macros));
}

#[test]
fn recursive_object_like_macro_collapses_to_false_in_if_expression() {
    let mut macros = HashMap::new();
    macros.insert("A".to_string(), object_macro("B"));
    macros.insert("B".to_string(), object_macro("A"));
    assert!(!eval_preproc_expr("A", &macros));
}
