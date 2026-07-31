//! Adversarial: the recursive-descent parser must fail closed on pathologically
//! nested input instead of overflowing the native stack and aborting the
//! process. A Rust stack overflow is an uncatchable SIGABRT, so these tests are
//! self-proving: if the depth guard regresses, the test binary crashes rather
//! than reporting a failure. Each input is far deeper than `MAX_PARSE_DEPTH`
//! (256) and would otherwise recurse tens of thousands of stack frames.
//!
//! These target the three recursion entry points that bypass each other:
//!  - `parse_expr` (paren nesting, the transitive recursion for all nesting),
//!  - `parse_unary` (`* ! &` prefix chains, right-recursive),
//!  - `parse_type` (`&mut &mut ... T`, right-recursive).

#![forbid(unsafe_code)]

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;

/// Lex then parse; return whether parsing returned (Ok or Err) without crashing.
/// The value asserted is that we get a `Result` back at all — reaching the
/// assertion proves no stack overflow occurred.
fn parses_without_crashing(src: &str) -> Result<(), ()> {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).map_err(|_| ())?;
    match parse(bytes, &tokens) {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }
}

#[test]
fn deeply_nested_parens_fail_closed() {
    // `fn f() -> i32 { return ((((...1...)))); }` with 20_000 paren pairs.
    let n = 20_000;
    let mut src = String::from("fn f() -> i32 { return ");
    src.push_str(&"(".repeat(n));
    src.push('1');
    src.push_str(&")".repeat(n));
    src.push_str("; }");
    // Must return a typed error (nesting too deep), not abort the process.
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep parens must be rejected with a ParseError, not accepted \
         (and crucially not crash the process)"
    );
}

#[test]
fn deeply_nested_unary_not_fails_closed() {
    // `!!!!...true` — right-recursive parse_unary chain.
    let mut src = String::from("fn f() -> bool { return ");
    src.push_str(&"!".repeat(20_000));
    src.push_str("true; }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep `!` chain must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_deref_fails_closed() {
    // `****...x` — right-recursive parse_unary via STAR.
    let mut src = String::from("fn f(x: i32) -> i32 { return ");
    src.push_str(&"*".repeat(20_000));
    src.push_str("x; }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep deref chain must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_borrow_expr_fails_closed() {
    // `&mut &mut ... x` — AMP_MUT tokens never pair (unlike `&`, which the
    // lexer greedily folds into `&&`), so this genuinely drives parse_unary's
    // borrow recursion deep. A plain `&` run would lex to `&&` and error
    // immediately without recursing, testing nothing — hence `&mut`.
    let mut src = String::from("fn f(x: i32) -> i32 { return ");
    src.push_str(&"&mut ".repeat(20_000));
    src.push_str("x; }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep `&mut` borrow chain must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_while_blocks_fail_closed() {
    // `while true { while true { ... {} ... } }` — the while body is parsed by a
    // direct parse_block call, a recursion cycle (parse_block -> parse_stmt ->
    // while-arm -> parse_block) that bypasses the expression/unary/type guards.
    // The block-level guard must catch it; otherwise this overflows the stack.
    let n = 20_000;
    let mut src = String::from("fn f() { ");
    src.push_str(&"while true { ".repeat(n));
    src.push_str(&"}".repeat(n));
    src.push_str(" }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep nested `while` blocks must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_if_blocks_fail_closed() {
    // `if true { if true { ... {} ... } }` as statements — another block-nesting
    // cycle through parse_block; must also fail closed.
    let n = 20_000;
    let mut src = String::from("fn f() { ");
    src.push_str(&"if true { ".repeat(n));
    src.push_str(&"}".repeat(n));
    src.push_str(" }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep nested `if` blocks must fail closed, not crash"
    );
}

#[test]
fn deeply_nested_ref_type_fails_closed() {
    // `&mut &mut ... i32` — right-recursive parse_type chain in a let binding.
    let mut src = String::from("fn f() { let x: ");
    src.push_str(&"&mut ".repeat(20_000));
    src.push_str("i32 = 0; }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Err(()),
        "Fix: 20k-deep `&mut` type chain must fail closed, not crash"
    );
}

#[test]
fn nesting_at_a_reasonable_depth_still_parses() {
    // A realistic depth (well under MAX_PARSE_DEPTH) must still be accepted, so
    // the guard does not over-reject legitimate programs. 32 paren pairs.
    let depth = 32;
    let mut src = String::from("fn f() -> i32 { return ");
    src.push_str(&"(".repeat(depth));
    src.push('1');
    src.push_str(&")".repeat(depth));
    src.push_str("; }");
    let r = parses_without_crashing(&src);
    assert_eq!(
        r,
        Ok(()),
        "Fix: a 32-deep paren nest is legal and must still parse"
    );
}
