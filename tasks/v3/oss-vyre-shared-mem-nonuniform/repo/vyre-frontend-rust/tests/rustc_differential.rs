//! Differential parity gate: our frontend's accept/reject verdict must match
//! real `rustc` on a curated nano-subset corpus.
//!
//! Our verdict = lex + parse + resolve + typeck + check_mutability + check_escape
//! all succeed. rustc verdict = `rustc --crate-type lib` compiles (lints capped
//! to allow, so only hard errors reject). The corpus is restricted to the rules
//! we implement (name resolution, type errors, E0596, E0597) plus clean
//! programs; conflicting-borrow programs (E0499/E0502) are excluded because that
//! rule is not yet wired (it would be a known divergence, not a bug).

#![forbid(unsafe_code)]

mod diff_support;
use diff_support::{ours_accepts, rustc_accepts};

/// Programs rustc accepts and we must accept.
const ACCEPT: &[&str] = &[
    "fn f() {}",
    "fn add(a: i32, b: i32) -> i32 { return a + b; }",
    "fn lt(a: i32, b: i32) -> bool { return a < b; }",
    "fn eq(a: i32, b: i32) -> bool { return a == b; }",
    "fn deref(r: &i32) -> i32 { return *r; }",
    "fn ret_ref(r: &i32) -> &i32 { return r; }",
    "fn deref_ref(r: &i32) -> &i32 { return &*r; }",
    "fn use_let() -> i32 { let x: i32 = 5; return x; }",
    "fn mut_let() -> i32 { let mut x: i32 = 5; return x; }",
    "fn branchy(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; }; }",
    "fn one() -> i32 { return 1; } fn g() -> i32 { return one(); }",
    "fn mut_borrow() { let mut x: i32 = 0; let r: &mut i32 = &mut x; }",
    "fn shared(x: i32) { let r: &i32 = &x; }",
    "fn arith() -> i32 { return 3 * 4 / 2 - 1 + 0; }",
    "fn boolret() -> bool { return true; }",
    "fn id(x: i32) -> i32 { return x; }",
    "fn two_lets() -> i32 { let a: i32 = 1; let b: i32 = a + 1; return b; }",
    "fn shadow(x: i32) -> i32 { let x: i32 = x + 1; return x; }",
    "fn deref_mut(r: &mut i32) -> i32 { return *r; }",
    "fn mutref(r: &mut i32) -> &mut i32 { return r; }",
    // Conflict-clean: two shared borrows coexist.
    "fn f() { let x: i32 = 0; let a: &i32 = &x; let b: &i32 = &x; let c: i32 = *a + *b; }",
    // Conflict-clean: sequential non-overlapping &mut borrows (NLL).
    "fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let p: i32 = *a; let b: &mut i32 = &mut x; let q: i32 = *b; }",
    // Conflict-clean: an unused first &mut is dead immediately (NLL).
    "fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; let c: i32 = *b; }",
    // Conflict-clean: &mut borrows confined to mutually exclusive branches.
    "fn f() { let mut x: i32 = 0; if true { let a: &mut i32 = &mut x; let p: i32 = *a; } else { let b: &mut i32 = &mut x; let q: i32 = *b; }; }",
    // Both arms return: the function diverges on every path.
    "fn diverge_if(a: i32) -> i32 { if a < 0 { return 0; } else { return a; }; }",
    // Nested if/else, all paths return.
    "fn nested_if(a: i32) -> i32 { if a < 0 { if a < 0 { return 1; } else { return 2; }; return 3; } else { return 4; }; }",
    // Nested calls, forward-referenced function.
    "fn nested_calls() -> i32 { return adder(adder(1, 2), 3); } fn adder(a: i32, b: i32) -> i32 { return a + b; }",
    // Passing `&mut` to a function (a temporary borrow, not a stored loan).
    "fn takes_mut(r: &mut i32) -> i32 { return *r; } fn caller(x: i32) -> i32 { let mut y: i32 = x; let z: i32 = takes_mut(&mut y); return z; }",
    // Half-open i32 range loops.
    "fn f(n: i32) -> i32 { let mut acc: i32 = 0; for i in 0..n { acc += i; } return acc; }",
    "fn f() -> i32 { let mut acc: i32 = 0; for i in -2..2 { acc += i; } return acc; }",
    // Range bounds resolve before the loop variable shadows an outer name.
    "fn f(i: i32) -> i32 { let mut acc: i32 = 0; for i in i..3 { acc += i; } return acc; }",
];

/// Programs rustc rejects and we must reject.
const REJECT: &[&str] = &[
    "fn f() -> i32 { return true; }",                                  // E0308
    "fn f() { let x: bool = 5; }",                                     // E0308
    "fn f() -> i32 { return missing; }",                               // E0425
    "fn f() -> i32 { return ghost(); }",                               // E0425
    "fn f() { let x: i32 = 0; let r: &mut i32 = &mut x; }",            // E0596
    "fn f(r: &i32) -> &i32 { let x: i32 = 0; return &x; }",            // E0515/E0597
    "fn f() -> i32 { let x: i32 = 0; return *x; }",                    // E0614
    "fn f() -> i32 { return true + 1; }",                              // E0308/E0277
    "fn f() -> i32 { let x: i32 = 0; }",                               // E0308 missing return
    "fn g(x: i32) -> i32 { return x; } fn f() -> i32 { return g(true); }", // E0308 arg
    "fn g(x: i32) -> i32 { return x; } fn f() -> i32 { return g(1, 2); }", // E0061 arity
    "fn f() { if 1 { let x: i32 = 0; }; }",                            // E0308 non-bool cond
    "fn f(x: i32) -> &i32 { return &x; }",                             // E0106/E0515
    // Two live &mut borrows of one place.
    "fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; let c: i32 = *a + *b; }", // E0499
    // A &mut while a shared borrow is still live.
    "fn f() { let mut x: i32 = 0; let a: &i32 = &x; let b: &mut i32 = &mut x; let c: i32 = *a; }",              // E0502
    // Two &mut live across a branch point (used in separate arms).
    "fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; if true { let p: i32 = *a; } else { let q: i32 = *b; }; }", // E0499
    "fn f() -> bool { return 1; }",                                    // E0308 i32 vs bool
    "fn f(a: i32) -> i32 { if a { return 1; } else { return 2; }; }",  // E0308 non-bool condition
    "fn f() -> i32 { let x: i32 = 0; if x < 1 { return x; }; }",       // E0308 missing return on fallthrough
    "fn unit_fn(x: i32) {} fn f() -> i32 { return unit_fn(1); }",      // E0308 return () where i32
    "fn f() -> i32 { for i in 0..3 {} return i; }",                    // E0425 loop variable out of scope
    "fn f() { for i in 0..3 { i = i + 1; } }",                         // E0384 loop variable is immutable
    "fn f() { for i in true..3 { let x: i32 = i; } }",                 // E0308 range start must be i32
];

/// Operator-coverage corpus: every binary/unary operator the parser, sema, and
/// lowering implement must be exercised against rustc, not just `<` and `==`.
/// The parser accepts `> <= >= != && || !` and lowering maps each to a distinct
/// IR op; before this corpus none of those seven were verified by any gate, so a
/// mistyped or mislowered operator would ship green. Each entry is a program
/// rustc accepts and we must accept.
const OPS_ACCEPT: &[&str] = &[
    "fn f(a: i32, b: i32) -> bool { return a > b; }",
    "fn f(a: i32, b: i32) -> bool { return a <= b; }",
    "fn f(a: i32, b: i32) -> bool { return a >= b; }",
    "fn f(a: i32, b: i32) -> bool { return a != b; }",
    "fn f(a: bool, b: bool) -> bool { return a && b; }",
    "fn f(a: bool, b: bool) -> bool { return a || b; }",
    "fn f(a: bool) -> bool { return !a; }",
    "fn f(a: bool, b: bool) -> bool { return a == b; }",
    "fn f(a: bool, b: bool) -> bool { return a != b; }",
    // Mixed precedence: comparisons under boolean connectives.
    "fn f(a: i32, b: i32) -> bool { return a > b && a != b; }",
    "fn f(a: i32, b: i32) -> bool { return !(a > b) || a == b; }",
    "fn f(a: bool, b: bool) -> bool { return !a && !b; }",
    "fn f(a: i32, b: i32) -> bool { return a >= b || a <= b; }",
    // Short-circuit chains (rustc evaluates left-to-right, RHS may be skipped).
    "fn f(a: bool, b: bool, c: bool) -> bool { return a && b || c; }",
    // Unary minus (arithmetic negation): literal, variable, and precedence.
    "fn f() -> i32 { return -5; }",
    "fn f(a: i32) -> i32 { return -a; }",
    "fn f(a: i32, b: i32) -> i32 { return -a + b; }", // parses as (-a) + b
    "fn f(a: i32) -> i32 { return -(-a); }",          // double negation
    // Compound assignment: desugars to `x = x <op> e` on a mutable binding.
    "fn f(a: i32) -> i32 { let mut x: i32 = a; x += 2; return x; }",
    "fn f(a: i32) -> i32 { let mut x: i32 = a; x -= 2; return x; }",
    "fn f(a: i32, b: i32) -> i32 { let mut x: i32 = a; x += b * 2; return x; }", // rhs is full expr
];

/// Operator-coverage REJECT corpus: type-incorrect uses of the operators that
/// rustc rejects (E0308 mismatched operand types) and we must reject.
const OPS_REJECT: &[&str] = &[
    "fn f(a: i32, b: bool) -> bool { return a && b; }", // && needs bool LHS
    "fn f(a: i32) -> bool { return a || a; }",          // || needs bool
    "fn f(a: i32, b: bool) -> bool { return a == b; }", // eq operands differ
    "fn f(a: i32, b: bool) -> bool { return a != b; }", // ne operands differ
    "fn f() -> bool { return !1; }",                    // !1 is i32 (bitwise), not bool
    "fn f() -> i32 { return true && false; }",          // bool result where i32 expected
    "fn f() -> i32 { return -true; }",                  // cannot apply unary `-` to bool (E0600)
    "fn f(a: i32) -> i32 { let x: i32 = a; x += 2; return x; }", // += to immutable (E0384)
];

/// Integer-literal boundary and overflow corpus. The parser stores literals via
/// `parse::<u64>().unwrap_or(0)`, so a literal exceeding u64 silently collapses
/// to 0, and nothing range-checks against the annotated type. rustc treats
/// over-i32 literals as the `overflowing_literals` lint (capped to allow here,
/// so ACCEPTED and wrapped), but a literal exceeding u128 is an unconditional
/// hard error (REJECTED) that no lint cap can suppress. This corpus pins both
/// the accepted-boundary cases and the hard-reject case so the silent-0 path
/// cannot diverge from rustc undetected.
const LITERAL_CORPUS: &[(&str, bool)] = &[
    ("fn f() -> i32 { return 0; }", true),
    ("fn f() -> i32 { return 2147483647; }", true), // i32::MAX
    // i32::MAX+1 and 2^32: overflowing_literals lint, capped to allow => rustc accepts (wraps).
    ("fn f() -> i32 { return 2147483648; }", true),
    ("fn f() -> i32 { return 4294967296; }", true),
    // u64::MAX: still within u128, lint-capped => accept.
    ("fn f() -> i32 { return 18446744073709551615; }", true),
    // 2^64: exceeds u64 but within u128 => lint-capped => rustc accepts.
    ("fn f() -> i32 { return 18446744073709551616; }", true),
    // u128::MAX: lint-capped => accept.
    (
        "fn f() -> i32 { return 340282366920938463463374607431768211455; }",
        true,
    ),
    // u128::MAX + 1: UNCONDITIONAL hard error "integer literal is too large".
    (
        "fn f() -> i32 { return 340282366920938463463374607431768211456; }",
        false,
    ),
];

#[test]
fn ours_agrees_with_rustc_on_operator_accept_corpus() {
    for (i, src) in OPS_ACCEPT.iter().enumerate() {
        let ours = ours_accepts(src);
        let rustc = rustc_accepts(src);
        assert!(
            rustc,
            "Fix: OPS_ACCEPT[{i}] must compile under rustc (adjust the corpus): {src}"
        );
        assert_eq!(
            ours, rustc,
            "Fix: operator verdict mismatch on OPS_ACCEPT[{i}] (ours={ours}, rustc={rustc}): {src}"
        );
    }
}

#[test]
fn ours_agrees_with_rustc_on_operator_reject_corpus() {
    for (i, src) in OPS_REJECT.iter().enumerate() {
        let ours = ours_accepts(src);
        let rustc = rustc_accepts(src);
        assert!(
            !rustc,
            "Fix: OPS_REJECT[{i}] must be rejected by rustc (adjust the corpus): {src}"
        );
        assert_eq!(
            ours, rustc,
            "Fix: operator verdict mismatch on OPS_REJECT[{i}] (ours={ours}, rustc={rustc}): {src}"
        );
    }
}

#[test]
fn ours_agrees_with_rustc_on_integer_literal_corpus() {
    for (i, (src, expect_accept)) in LITERAL_CORPUS.iter().enumerate() {
        let rustc = rustc_accepts(src);
        assert_eq!(
            rustc, *expect_accept,
            "Fix: LITERAL_CORPUS[{i}] rustc verdict ({rustc}) != expected ({expect_accept}); \
             rustc behavior may have shifted (adjust the corpus): {src}"
        );
        let ours = ours_accepts(src);
        assert_eq!(
            ours, rustc,
            "Fix: literal verdict mismatch on LITERAL_CORPUS[{i}] (ours={ours}, rustc={rustc}): {src}"
        );
    }
}

#[test]
fn ours_agrees_with_rustc_on_accept_corpus() {
    for (i, src) in ACCEPT.iter().enumerate() {
        let ours = ours_accepts(src);
        let rustc = rustc_accepts(src);
        assert!(
            rustc,
            "Fix: ACCEPT corpus[{i}] must compile under rustc (adjust the corpus): {src}"
        );
        assert_eq!(
            ours, rustc,
            "Fix: verdict mismatch on ACCEPT corpus[{i}] (ours={ours}, rustc={rustc}): {src}"
        );
    }
}

#[test]
fn ours_agrees_with_rustc_on_reject_corpus() {
    for (i, src) in REJECT.iter().enumerate() {
        let ours = ours_accepts(src);
        let rustc = rustc_accepts(src);
        assert!(
            !rustc,
            "Fix: REJECT corpus[{i}] must be rejected by rustc (adjust the corpus): {src}"
        );
        assert_eq!(
            ours, rustc,
            "Fix: verdict mismatch on REJECT corpus[{i}] (ours={ours}, rustc={rustc}): {src}"
        );
    }
}
