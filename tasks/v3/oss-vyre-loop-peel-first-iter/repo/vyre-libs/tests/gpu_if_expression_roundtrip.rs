//! GPU `#if` / `#elif` expression evaluator reference roundtrip.
//!
//! Drives the kernel for each `#if` / `#elif` expression case and
//! asserts the emitted `directive_values[i]` row matches the CPU
//! `reference_c_preprocessor_directive_metadata`. Other directive
//! kinds must remain `0` in this kernel's output column.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod support;

use support::gpu_if_expression::{
    assert_gpu_matches_cpu, run_full_pipeline, run_if_expression_with_macro_value,
};

#[test]
fn bare_identifier_uses_object_like_macro_value_not_definedness() {
    assert_eq!(run_if_expression_with_macro_value(b"#if F\n", b"F", 0), 0);
    assert_eq!(
        run_if_expression_with_macro_value(b"#if F == 7\n", b"F", 7),
        1
    );
}

#[test]
fn long_bare_identifier_uses_full_macro_name() {
    let name = format!("CONFIG_{}_VALUE", "LONG_".repeat(40));
    let source = format!("#if {name} == 7\n");

    assert_eq!(
        run_if_expression_with_macro_value(source.as_bytes(), name.as_bytes(), 7),
        1
    );
}

#[test]
fn long_defined_identifier_uses_full_macro_name() {
    let name = format!("HAVE_{}_FEATURE", "GENERATED_".repeat(32));
    let source = format!("#if defined({name})\n");
    let defined = [name.as_bytes()];

    assert_gpu_matches_cpu(source.as_bytes(), &defined);
}

#[test]
fn long_if_payload_scans_past_old_512_byte_cap() {
    let mut source = String::from("#if 0");
    for _ in 0..180 {
        source.push_str(" || 0");
    }
    source.push_str(" || 1\n");

    assert!(
        source.len() > 512,
        "test must cross the old fixed payload scan cap"
    );
    assert_gpu_matches_cpu(source.as_bytes(), &[]);
}

// ---- Literal-only ----

#[test]
fn if_one() {
    assert_gpu_matches_cpu(b"#if 1\n", &[]);
}

#[test]
fn if_zero() {
    assert_gpu_matches_cpu(b"#if 0\n", &[]);
}

#[test]
fn if_decimal_value() {
    assert_gpu_matches_cpu(b"#if 42\n", &[]);
}

#[test]
fn if_hex_value() {
    assert_gpu_matches_cpu(b"#if 0x1\n", &[]);
}

#[test]
fn if_octal_value() {
    assert_gpu_matches_cpu(b"#if 010\n", &[]);
}

// ---- Arithmetic ----

#[test]
fn if_addition() {
    assert_gpu_matches_cpu(b"#if 2 + 3\n", &[]);
}

#[test]
fn if_subtraction() {
    assert_gpu_matches_cpu(b"#if 5 - 5\n", &[]);
}

#[test]
fn if_multiplication() {
    assert_gpu_matches_cpu(b"#if 2 * 3\n", &[]);
}

#[test]
fn if_division_nonzero() {
    assert_gpu_matches_cpu(b"#if 8 / 4\n", &[]);
}

#[test]
fn if_remainder_nonzero() {
    assert_gpu_matches_cpu(b"#if 9 % 4\n", &[]);
}

#[test]
fn if_precedence_mul_over_add() {
    // 2 + 3 * 4 = 14, not 20.
    assert_gpu_matches_cpu(b"#if 2 + 3 * 4\n", &[]);
}

#[test]
fn if_precedence_with_parens() {
    // (2 + 3) * 4 = 20.
    assert_gpu_matches_cpu(b"#if (2 + 3) * 4\n", &[]);
}

// ---- Comparison ----

#[test]
fn if_equal_true() {
    assert_gpu_matches_cpu(b"#if 1 == 1\n", &[]);
}

#[test]
fn if_equal_false() {
    assert_gpu_matches_cpu(b"#if 1 == 2\n", &[]);
}

#[test]
fn if_not_equal() {
    assert_gpu_matches_cpu(b"#if 1 != 2\n", &[]);
}

#[test]
fn if_less_than() {
    assert_gpu_matches_cpu(b"#if 1 < 2\n", &[]);
}

#[test]
fn if_less_or_equal() {
    assert_gpu_matches_cpu(b"#if 2 <= 2\n", &[]);
}

#[test]
fn if_greater_than() {
    assert_gpu_matches_cpu(b"#if 3 > 2\n", &[]);
}

#[test]
fn if_greater_or_equal() {
    assert_gpu_matches_cpu(b"#if 3 >= 3\n", &[]);
}

// ---- Logical ----

#[test]
fn if_logical_and_true() {
    assert_gpu_matches_cpu(b"#if 1 && 1\n", &[]);
}

#[test]
fn if_logical_and_false() {
    assert_gpu_matches_cpu(b"#if 1 && 0\n", &[]);
}

#[test]
fn if_logical_or_true() {
    assert_gpu_matches_cpu(b"#if 1 || 0\n", &[]);
}

#[test]
fn if_logical_or_false() {
    assert_gpu_matches_cpu(b"#if 0 || 0\n", &[]);
}

// ---- Bitwise ----

#[test]
fn if_bitwise_and() {
    assert_gpu_matches_cpu(b"#if 0xff & 0x0f\n", &[]);
}

#[test]
fn if_bitwise_or() {
    assert_gpu_matches_cpu(b"#if 0xf0 | 0x0f\n", &[]);
}

#[test]
fn if_bitwise_xor() {
    assert_gpu_matches_cpu(b"#if 0xff ^ 0x0f\n", &[]);
}

#[test]
fn if_left_shift() {
    assert_gpu_matches_cpu(b"#if 1 << 4\n", &[]);
}

#[test]
fn if_right_shift() {
    assert_gpu_matches_cpu(b"#if 16 >> 2\n", &[]);
}

// ---- Unary ----

#[test]
fn if_logical_not() {
    assert_gpu_matches_cpu(b"#if !0\n", &[]);
}

#[test]
fn if_logical_not_double() {
    assert_gpu_matches_cpu(b"#if !!1\n", &[]);
}

#[test]

fn if_unary_minus() {
    // -1 != 0 → true.
    assert_gpu_matches_cpu(b"#if -1\n", &[]);
}

// ---- defined() ----

#[test]
fn if_defined_paren_form_when_defined() {
    assert_gpu_matches_cpu(b"#if defined(FOO)\n", &[b"FOO".as_slice()]);
}

#[test]
fn if_defined_paren_form_when_undefined() {
    assert_gpu_matches_cpu(b"#if defined(MISSING)\n", &[b"FOO".as_slice()]);
}

#[test]
fn if_defined_no_paren_form() {
    assert_gpu_matches_cpu(b"#if defined FOO\n", &[b"FOO".as_slice()]);
}

#[test]
fn if_defined_combined_with_logical() {
    assert_gpu_matches_cpu(
        b"#if defined(FOO) && defined(BAR)\n",
        &[b"FOO".as_slice(), b"BAR".as_slice()],
    );
}

#[test]
fn if_defined_negated() {
    assert_gpu_matches_cpu(b"#if !defined(MISSING)\n", &[]);
}

// ---- Bare macro reference ----

#[test]
fn if_bare_ident_when_defined_is_one() {
    assert_gpu_matches_cpu(b"#if FOO\n", &[b"FOO".as_slice()]);
}

#[test]
fn if_bare_ident_when_undefined_is_zero() {
    assert_gpu_matches_cpu(b"#if MISSING\n", &[]);
}

// ---- Mixed ----

#[test]
fn if_complex_kernel_idiom_guard() {
    assert_gpu_matches_cpu(
        b"#if defined(__GNUC__) && (__GNUC__ >= 4)\n",
        &[b"__GNUC__".as_slice()],
    );
}

#[test]
fn if_has_builtin_matches_cpu_metadata() {
    assert_gpu_matches_cpu(b"#if __has_builtin(__builtin_expect)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_trap)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_unreachable)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_alloca)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_bswap64)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_isnan)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_va_start)\n", &[]);
    assert_gpu_matches_cpu(b"#if __has_builtin(__builtin_vyre_unknown)\n", &[]);
    assert_gpu_matches_cpu(b"#if 1 && __has_builtin(__builtin_allocax)\n", &[]);
}

#[test]
fn if_generic_has_operators_match_cpu_metadata() {
    assert_gpu_matches_cpu(b"#if !__has_attribute(visibility)\n", &[]);
    assert_gpu_matches_cpu(b"#if __has_feature(c_static_assert)\n", &[]);
    assert_gpu_matches_cpu(b"#if __has_include(<linux/types.h>)\n", &[]);
}

#[test]
fn elif_basic() {
    let src = b"#if 0\n#elif 1\n#endif\n";
    assert_gpu_matches_cpu(src, &[]);
}

#[test]
fn other_directive_kinds_emit_zero_in_value_column() {
    let src = b"#define X 1\n#include <foo.h>\n#pragma once\n";
    let (_kinds, gpu_values) = run_full_pipeline(src, &[]);
    assert!(
        gpu_values.iter().all(|&v| v == 0),
        "non-if/elif rows must emit 0; got {gpu_values:?}"
    );
}
