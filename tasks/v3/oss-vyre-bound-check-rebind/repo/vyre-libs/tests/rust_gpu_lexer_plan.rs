//! Reference-oracle coverage for the Rust GPU lexer plan.

#![cfg(feature = "rust-parser")]
#![forbid(unsafe_code)]

use vyre_libs::parsing::rust::lex::lexer::core::{lex as lex_cpu, Token};
use vyre_libs::parsing::rust::lex::lexer::plan::{rust_lexer, rust_lexer_batch, RustLexerPlan};
use vyre_libs::parsing::rust::lex::tokens::*;
use vyre_reference::value::Value;

fn u32_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn decode_u32_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 output chunk")))
        .collect()
}

fn source_words(source: &[u8]) -> Vec<u32> {
    let mut words: Vec<u32> = source.iter().map(|byte| u32::from(*byte)).collect();
    if words.is_empty() {
        words.push(0);
    }
    words
}

fn gpu_lex(source: &[u8]) -> Vec<Token> {
    let haystack_len = source.len() as u32;
    let program = rust_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_len,
    );
    let token_capacity = source.len().saturating_add(1).max(1);
    let zero_tokens = vec![0u8; token_capacity * 4];
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&source_words(source))),
            Value::from(zero_tokens.clone()),
            Value::from(zero_tokens.clone()),
            Value::from(zero_tokens),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("Rust GPU lexer plan must execute under the reference oracle");
    assert_eq!(
        outputs.len(),
        4,
        "Rust GPU lexer must emit [types, starts, lens, count]"
    );
    let kinds = decode_u32_words(&outputs[0].to_bytes());
    let starts = decode_u32_words(&outputs[1].to_bytes());
    let lens = decode_u32_words(&outputs[2].to_bytes());
    let count = decode_u32_words(&outputs[3].to_bytes())
        .first()
        .copied()
        .expect("count word") as usize;
    assert!(
        count <= kinds.len() && count <= starts.len() && count <= lens.len(),
        "emitted token count must fit every output column"
    );
    (0..count)
        .map(|idx| Token {
            kind: u16::try_from(kinds[idx]).expect("token kind fits u16"),
            start: starts[idx],
            len: u16::try_from(lens[idx]).expect("token length fits u16"),
        })
        .collect()
}

fn gpu_lex_batch(sources: &[&[u8]]) -> Vec<Vec<Token>> {
    let mut packed = Vec::new();
    let mut offsets = Vec::with_capacity(sources.len());
    let mut lens = Vec::with_capacity(sources.len());
    for source in sources {
        offsets.push(packed.len() as u32);
        lens.push(source.len() as u32);
        packed.extend_from_slice(source);
    }

    let source_count = sources.len() as u32;
    let haystack_len = packed.len() as u32;
    let token_stride = sources
        .iter()
        .map(|source| source.len().saturating_add(1).max(1))
        .max()
        .unwrap_or(1) as u32;
    let token_slots = usize::try_from(source_count)
        .expect("source count fits usize")
        .saturating_mul(usize::try_from(token_stride).expect("token stride fits usize"));

    let program = rust_lexer_batch(
        "haystack",
        "source_offsets",
        "source_lens",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_len,
        source_count,
        token_stride,
    );
    let zero_tokens = vec![0u8; token_slots * 4];
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&source_words(&packed))),
            Value::from(u32_bytes(&offsets)),
            Value::from(u32_bytes(&lens)),
            Value::from(zero_tokens.clone()),
            Value::from(zero_tokens.clone()),
            Value::from(zero_tokens),
            Value::from(vec![0u8; sources.len().max(1) * 4]),
        ],
    )
    .expect("batched Rust GPU lexer plan must execute under the reference oracle");
    assert_eq!(
        outputs.len(),
        4,
        "batched Rust GPU lexer must emit [types, starts, lens, counts]"
    );
    let kinds = decode_u32_words(&outputs[0].to_bytes());
    let starts = decode_u32_words(&outputs[1].to_bytes());
    let lens = decode_u32_words(&outputs[2].to_bytes());
    let counts = decode_u32_words(&outputs[3].to_bytes());

    sources
        .iter()
        .enumerate()
        .map(|(source_idx, _)| {
            let count = counts[source_idx] as usize;
            assert!(
                count <= token_stride as usize,
                "source {source_idx} emitted {count} tokens beyond stride {token_stride}"
            );
            let base = source_idx * token_stride as usize;
            (0..count)
                .map(|token_idx| {
                    let out_idx = base + token_idx;
                    Token {
                        kind: u16::try_from(kinds[out_idx]).expect("token kind fits u16"),
                        start: starts[out_idx],
                        len: u16::try_from(lens[out_idx]).expect("token length fits u16"),
                    }
                })
                .collect()
        })
        .collect()
}

fn assert_gpu_matches_cpu(source: &str) {
    let cpu = lex_cpu(source.as_bytes()).expect("CPU lexer must accept fixture");
    let gpu = gpu_lex(source.as_bytes());
    assert_eq!(gpu, cpu, "GPU lexer diverged from CPU lexer for:\n{source}");
}

#[test]
fn gpu_lexer_matches_cpu_on_frontend_subset_corpus() {
    let corpus = [
        "",
        "fn f() {}",
        "fn add(a: i32, b: i32) -> i32 { return a + b; }",
        "fn branchy(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; }; }",
        "fn f(n: i32) -> i32 { let mut acc: i32 = 0; for i in -3..n { acc += i; } return acc; }",
        "fn f(a: i32) -> i32 { let r: &i32 = &a; return *r - -1; }",
        "fn f(a: bool, b: bool) -> bool { return !a && b || false; }",
        "fn f() -> i32 { /* block */ let x: i32 = 1; // line\n return x; }",
        "fn f(x: i32) -> i32 {\n\tlet mut y: i32 = x;\r\n\ty -= 2;\n\treturn y % 3;\n}",
    ];
    for source in corpus {
        assert_gpu_matches_cpu(source);
    }
}

#[test]
fn gpu_batch_lexer_matches_cpu_per_source_on_frontend_subset_corpus() {
    let corpus: [&[u8]; 7] = [
        b"",
        b"fn f() {}",
        b"fn add(a: i32, b: i32) -> i32 { return a + b; }",
        b"fn branchy(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; }; }",
        b"fn f(n: i32) -> i32 { let mut acc: i32 = 0; for i in -3..n { acc += i; } return acc; }",
        b"fn f(a: bool, b: bool) -> bool { return !a && b || false; }",
        b"fn f() -> i32 { /* block */ let x: i32 = 1; // line\n return x; }",
    ];
    let gpu = gpu_lex_batch(&corpus);
    assert_eq!(gpu.len(), corpus.len());
    for (source_idx, source) in corpus.iter().enumerate() {
        let cpu = lex_cpu(source).expect("CPU lexer must accept batch fixture");
        assert_eq!(
            gpu[source_idx], cpu,
            "batched GPU lexer diverged from CPU lexer for source {source_idx}"
        );
    }
}

#[test]
fn gpu_lexer_emits_error_token_for_unknown_byte() {
    let tokens = gpu_lex(b"fn f() { @ }");
    assert!(
        tokens.iter().any(|token| token.kind == ERROR),
        "unknown byte must surface as an ERROR token instead of disappearing"
    );
}

#[test]
fn gpu_batch_lexer_keeps_unknown_byte_error_in_own_source_window() {
    let corpus: [&[u8]; 3] = [
        b"fn ok() {}",
        b"fn bad() { @ }",
        b"fn also_ok() -> i32 { return 7; }",
    ];
    let gpu = gpu_lex_batch(&corpus);

    assert!(
        gpu[1]
            .iter()
            .any(|token| token.kind == ERROR && token.start == 11),
        "unknown byte in source 1 must surface as a source-relative ERROR token"
    );
    assert!(
        !gpu[0].iter().any(|token| token.kind == ERROR)
            && !gpu[2].iter().any(|token| token.kind == ERROR),
        "one source's ERROR token must not bleed into neighboring source windows"
    );
}

#[test]
fn plan_builder_legacy_build_emits_empty_source_lexer() {
    let program = RustLexerPlan::new().build();
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&source_words(b""))),
            Value::from(vec![0u8; 4]),
            Value::from(vec![0u8; 4]),
            Value::from(vec![0u8; 4]),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("legacy no-arg RustLexerPlan::build must emit an executable empty-source plan");
    assert_eq!(decode_u32_words(&outputs[0].to_bytes())[0], u32::from(EOF));
    assert_eq!(decode_u32_words(&outputs[3].to_bytes())[0], 1);
}
