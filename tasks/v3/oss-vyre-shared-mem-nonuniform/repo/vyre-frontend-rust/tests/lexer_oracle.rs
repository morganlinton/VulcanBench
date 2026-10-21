//! Promotion-gate (lexer correctness): the reusable Rust lexer substrate must
//! agree with `rustc_lexer`, byte-for-byte over content, across a nano-subset
//! corpus. Listed in MATURITY.md as the lexer-correctness gate.

#![forbid(unsafe_code)]

mod oracle_support;
use oracle_support::{lexer_parity, OracleResult};

const CORPUS: &[&str] = &[
    "fn main() { let x: i32 = 5; }",
    "fn add(a: i32, b: i32) -> i32 { return a + b; }",
    "fn max(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; }; }",
    "fn deref(x: &i32) -> i32 { return *x; }",
    "fn borrow_mut(x: &mut i32) -> bool { let mut ok: bool = true; return ok; }",
    "fn arith(a: i32) -> i32 { return a * 3 / 1 - 2 + 0; }",
    "fn eq(a: i32, b: i32) -> bool { return a == b; }",
    "fn neg(a: i32) -> i32 { return -a; }",
    "// leading comment\nfn empty() { /* body */ }",
];

#[test]
fn substrate_lexer_matches_rustc_over_corpus() {
    for (i, src) in CORPUS.iter().enumerate() {
        match lexer_parity(src.as_bytes()) {
            OracleResult::Match => {}
            OracleResult::Mismatch(why) => panic!(
                "Fix: substrate lexer diverged from rustc_lexer on corpus[{i}] {src:?}: {why}"
            ),
        }
    }
}
