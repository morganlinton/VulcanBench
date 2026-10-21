use super::*;

fn eval(src: &[u8], defined: &[&[u8]]) -> Result<bool, CPreprocessorError> {
    PreprocessorExprParser {
        bytes: src,
        index: 0,
        base_offset: 0,
        defined_macros: defined,
        depth: 0,
    }
    .parse()
}

// --- Adversarial: #if expression recursion must fail closed, not overflow ---
// The conditional evaluator is recursive descent; without a depth bound,
// hostile nesting overflows the native stack and aborts the process (an
// uncatchable SIGABRT). These tests are self-proving: if the guard regresses,
// the test binary crashes rather than reporting a failure. Each input is far
// deeper than MAX_PP_EXPR_DEPTH (256).

#[test]
fn deeply_nested_parens_fail_closed() {
    // `((((...1...))))` routes parse_unary -> parse_conditional per level.
    let n = 50_000;
    let mut src = Vec::new();
    src.extend(std::iter::repeat(b'(').take(n));
    src.push(b'1');
    src.extend(std::iter::repeat(b')').take(n));
    let r = eval(&src, &[]);
    assert_eq!(
        r.is_err(),
        true,
        "Fix: 50k-deep #if parens must be rejected with an error, not crash"
    );
}

#[test]
fn deeply_nested_unary_not_fails_closed() {
    // `!!!!...1` right-recurses parse_unary.
    let mut src = vec![b'!'; 50_000];
    src.push(b'1');
    let r = eval(&src, &[]);
    assert_eq!(
        r.is_err(),
        true,
        "Fix: 50k-deep #if `!` chain must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_unary_minus_fails_closed() {
    // `----...1` right-recurses parse_unary via the negation arm.
    let mut src = vec![b'-'; 50_000];
    src.push(b'1');
    let r = eval(&src, &[]);
    assert_eq!(
        r.is_err(),
        true,
        "Fix: 50k-deep #if `-` chain must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_bitnot_fails_closed() {
    // `~~~~...1` right-recurses parse_unary via the complement arm.
    let mut src = vec![b'~'; 50_000];
    src.push(b'1');
    let r = eval(&src, &[]);
    assert_eq!(
        r.is_err(),
        true,
        "Fix: 50k-deep #if `~` chain must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_ternary_fails_closed() {
    // `1?1:1?1:1?...:1` right-recurses parse_conditional through the else arm.
    let n = 50_000;
    let mut src = Vec::new();
    for _ in 0..n {
        src.extend_from_slice(b"1?1:");
    }
    src.push(b'1');
    let r = eval(&src, &[]);
    assert_eq!(
        r.is_err(),
        true,
        "Fix: 50k-deep #if ternary chain must fail closed, not crash"
    );
}

#[test]
fn reasonable_nesting_still_evaluates() {
    // Well under MAX_PP_EXPR_DEPTH: the guard must not over-reject. 32 parens
    // around a true expression must still evaluate to true.
    let depth = 32;
    let mut src = Vec::new();
    src.extend(std::iter::repeat(b'(').take(depth));
    src.extend_from_slice(b"1");
    src.extend(std::iter::repeat(b')').take(depth));
    let r = eval(&src, &[]);
    assert_eq!(
        r,
        Ok(true),
        "Fix: a 32-deep parenthesized #if expression is legal and must evaluate"
    );
}

// --- Property: the #if evaluator is total over all inputs ---
// Robustness invariant (mirrors vyre-frontend-rust/tests/proptest_robustness.rs
// for the C side): for ANY byte sequence, the conditional-expression evaluator
// must return a `Result` (Ok or Err) without panicking — no unwrap blow-up, no
// arithmetic overflow panic, no slice index out of bounds, no stack overflow.
// The depth guard (MAX_PP_EXPR_DEPTH) bounds recursion so the only remaining
// failure modes are catchable panics, which proptest's harness surfaces and
// shrinks. We also assert determinism: a pure function over `&[u8]` must give
// the same verdict twice (catches accidental dependence on hidden state).
mod robustness {
    use super::eval;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        /// Fully arbitrary bytes (length-bounded so recursion stays well within
        /// the depth guard) must never crash the evaluator.
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let first = eval(&bytes, &[]);
            let second = eval(&bytes, &[]);
            prop_assert_eq!(
                first.is_ok(),
                second.is_ok(),
                "evaluator must be deterministic over identical input"
            );
        }

        /// Bytes drawn from the `#if` expression alphabet drive deep into the
        /// arithmetic/recursion paths (most random bytes error out early at an
        /// unexpected operand). This is the high-yield generator for exercising
        /// the operator/precedence/ternary logic without panicking.
        #[test]
        fn expression_soup_never_panics(
            tokens in proptest::collection::vec(
                proptest::sample::select(
                    &b"0123456789()+-*/%!~&|^<>=?: \tabcdefABCDEF"[..]
                ),
                0..256,
            )
        ) {
            let first = eval(&tokens, &[b"A".as_slice(), b"B".as_slice()]);
            let second = eval(&tokens, &[b"A".as_slice(), b"B".as_slice()]);
            prop_assert_eq!(
                first.is_ok(),
                second.is_ok(),
                "evaluator must be deterministic over identical expression soup"
            );
        }
    }
}

#[test]
fn has_attribute_returns_zero_and_consumes_argument() {
    // Real glibc / kernel pattern: gate attribute application on a
    // __has_attribute probe. We don't promise any attribute, so the
    // probe must evaluate to 0 (false) AND fully consume its argument
    // so the directive parses cleanly.
    assert_eq!(eval(b"__has_attribute(visibility)", &[]), Ok(false));
    assert_eq!(eval(b"__has_feature(c_static_assert)", &[]), Ok(false));
    assert_eq!(eval(b"__has_extension(c_alignof)", &[]), Ok(false));
    assert_eq!(eval(b"__has_warning(\"-Wfoo\")", &[]), Ok(false));
}

#[test]
fn has_builtin_uses_frontend_builtin_catalog() {
    assert_eq!(eval(b"__has_builtin(__builtin_expect)", &[]), Ok(true));
    assert_eq!(eval(b"__has_builtin(__builtin_popcount)", &[]), Ok(true));
    assert_eq!(
        eval(b"__has_constexpr_builtin(__builtin_bitreverse32)", &[]),
        Ok(true)
    );
    assert_eq!(
        eval(b"__has_builtin(__builtin_vyre_unknown)", &[]),
        Ok(false)
    );
    assert_eq!(eval(b"__has_builtin(ordinary_identifier)", &[]), Ok(false));
}

#[test]
fn has_attribute_inside_or_chain() {
    // `#if !__has_attribute(x) || defined(FALLBACK)`  -  the OR short-circuit
    // requires the right-hand side to evaluate too.
    assert_eq!(
        eval(
            b"!__has_attribute(visibility) || defined(FALLBACK)",
            &[b"FALLBACK"]
        ),
        Ok(true)
    );
    assert_eq!(
        eval(b"!__has_attribute(visibility) || defined(FALLBACK)", &[]),
        Ok(true)
    );
}

#[test]
fn has_c_attribute_with_scoped_name() {
    // C23 / C++ scoped attribute name: `__has_c_attribute(gnu::packed)`.
    // We must consume `vendor::name` syntactically and still return 0.
    assert_eq!(eval(b"__has_c_attribute(gnu::packed)", &[]), Ok(false));
    assert_eq!(
        eval(b"__has_cpp_attribute(clang::trivial_abi)", &[]),
        Ok(false)
    );
}

#[test]
fn is_identifier_matches_keyword_guard_semantics() {
    assert_eq!(eval(b"__is_identifier(regular_name)", &[]), Ok(true));
    assert_eq!(eval(b"__is_identifier(__int128)", &[]), Ok(false));
    assert_eq!(eval(b"__is_identifier(typeof)", &[]), Ok(false));
    assert_eq!(eval(b"__is_identifier(_Static_assert)", &[]), Ok(false));
}

#[test]
fn has_include_angle_and_quoted() {
    // Real defensive header pattern:
    //   #if __has_include(<threads.h>)
    //     #include <threads.h>
    //   #else
    //     ...
    //   #endif
    assert_eq!(eval(b"__has_include(<threads.h>)", &[]), Ok(false));
    assert_eq!(eval(b"__has_include(\"local.h\")", &[]), Ok(false));
    assert_eq!(eval(b"__has_include_next(<stdio.h>)", &[]), Ok(false));
}

#[test]
fn has_embed_consumes_resource_and_parameters() {
    assert_eq!(eval(b"__has_embed(<asset.bin>)", &[]), Ok(false));
    assert_eq!(eval(b"__has_embed(\"asset.bin\" limit(4))", &[]), Ok(false));
    assert_eq!(
        eval(b"__has_embed(__FILE__ limit (4) vendor::attr(42))", &[]),
        Ok(false)
    );
}

#[test]
fn has_include_rejects_unterminated_header() {
    // An unterminated angle-form header must produce a Fix: diagnostic,
    // not silently swallow the rest of the directive.
    let err = eval(b"__has_include(<threads.h", &[]).expect_err("unterminated header");
    assert!(
        err.to_string().contains("close __has_include header"),
        "unterminated header error: {err}"
    );
}

#[test]
fn has_embed_rejects_unterminated_operands() {
    let err_angle = eval(b"__has_embed(<asset.bin", &[]).expect_err("unterminated angle resource");
    assert!(
        err_angle.to_string().contains("close __has_embed resource"),
        "unterminated embed angle error: {err_angle}"
    );
    let err_paren =
        eval(b"__has_embed(\"asset.bin\" limit(4)", &[]).expect_err("unterminated embed tail");
    assert!(
        err_paren.to_string().contains("close __has_embed operator"),
        "unterminated embed paren error: {err_paren}"
    );
}

#[test]
fn has_attribute_rejects_missing_paren() {
    // `__has_attribute visibility` (no parens) is a malformed directive  -
    // we must reject it rather than silently treating it as 0.
    let err = eval(b"__has_attribute visibility", &[]).expect_err("missing paren");
    assert!(
        err.to_string().contains("parenthesized argument"),
        "missing-paren __has_attribute error: {err}"
    );
}

#[test]
fn has_attribute_does_not_collide_with_macro_lookup() {
    // If the user `#define __has_attribute 1` (some headers do this as a
    // polyfill on non-clang compilers), the operator-form must still
    // parse  -  we don't currently honor the user's override, but we MUST
    // continue to consume the `(name)` payload so the directive is valid.
    assert_eq!(
        eval(b"__has_attribute(visibility)", &[b"__has_attribute"]),
        Ok(false)
    );
}

#[test]
fn modern_integer_literals_consume_digit_separators_and_suffixes() {
    assert_eq!(eval(b"1'024ULL == 1024", &[]), Ok(true));
    assert_eq!(eval(b"0xFF'00z == 65280", &[]), Ok(true));
    assert_eq!(eval(b"0b1010'0101WB == 165", &[]), Ok(true));
    assert_eq!(eval(b"0755'1uL > 0", &[]), Ok(true));
}
