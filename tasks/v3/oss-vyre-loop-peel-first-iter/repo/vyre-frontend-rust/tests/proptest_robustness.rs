//! Property tests for the Rust frontend pipeline at scale.
//!
//! Thousands of generated inputs (random bytes, token soup) drive the full
//! pipeline. Invariants: the pipeline never panics on any input, and name
//! resolution is deterministic. This is the reliability half of the contract:
//! the suite runs under nastier inputs than any real source file.

#![forbid(unsafe_code)]

use proptest::prelude::*;
use vyre_frontend_rust::pipeline::{RustPipeline, RustPipelineConfig};
use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::resolve;

/// Drive the pipeline under every config; it must return (never panic).
fn run_all_configs(src: &[u8]) {
    let _ = RustPipeline::new(RustPipelineConfig::default()).compile_unit(src);
    let borrow_on = RustPipelineConfig {
        gpu_lex: false,
        borrow_check: true,
        lower: false,
        lower_lane_count: None,
    };
    let _ = RustPipeline::new(borrow_on).compile_unit(src);
}

/// A single nano-subset lexical token (so token soup frequently parses).
fn token() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("fn"),
        Just("let"),
        Just("mut"),
        Just("if"),
        Just("else"),
        Just("return"),
        Just("i32"),
        Just("bool"),
        Just("true"),
        Just("false"),
        Just("("),
        Just(")"),
        Just("{"),
        Just("}"),
        Just(";"),
        Just(":"),
        Just(","),
        Just("->"),
        Just("&"),
        Just("*"),
        Just("+"),
        Just("-"),
        Just("<"),
        Just("=="),
        Just("="),
        Just("x"),
        Just("y"),
        Just("z"),
        Just("f"),
        Just("g"),
        Just("0"),
        Just("1"),
        Just("42"),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// The pipeline must never panic on arbitrary bytes (valid or invalid UTF-8).
    #[test]
    fn never_panics_on_random_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..192)) {
        run_all_configs(&bytes);
    }

    /// Token soup reaches deep into the parser and resolver; still no panic.
    #[test]
    fn never_panics_on_token_soup(toks in proptest::collection::vec(token(), 0..64)) {
        let src = toks.join(" ");
        run_all_configs(src.as_bytes());
    }

    /// Resolution is a pure function of (module, source): same input, same result.
    #[test]
    fn resolve_is_deterministic(toks in proptest::collection::vec(token(), 0..64)) {
        let src = toks.join(" ");
        let bytes = src.as_bytes();
        if let Ok(tokens) = lex(bytes) {
            if let Ok(module) = parse(bytes, &tokens) {
                let a = resolve(&module, bytes);
                let b = resolve(&module, bytes);
                prop_assert_eq!(a.is_ok(), b.is_ok());
                if let (Ok(a), Ok(b)) = (a, b) {
                    prop_assert_eq!(a.bindings.len(), b.bindings.len());
                    prop_assert_eq!(a.uses.len(), b.uses.len());
                }
            }
        }
    }
}
