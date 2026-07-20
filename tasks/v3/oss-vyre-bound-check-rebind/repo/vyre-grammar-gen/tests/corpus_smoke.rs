//! Curated snippets: preprocess + fast stub lexer counts.

use vyre_grammar_gen::count_chunked_valid_tokens;
use vyre_grammar_gen::host_preprocess::preprocess_c_host;
use vyre_grammar_gen::DfaBuilder;

const HELLO: &str = include_str!("../corpus/hello.c");
const COMMENT: &str = include_str!("../corpus/comment_only.c");

#[test]
fn preprocess_strips_block_comment_corpus() {
    let p = preprocess_c_host(COMMENT);
    assert!(!p.contains("empty"));
}

#[test]
fn hello_c_chunk_count_is_stable_on_stub_dfa() {
    let mut b = DfaBuilder::new(2, 256);
    for c in 0u32..256 {
        b.continue_to(0, c, 1).expect("state 1 fits in u16");
        b.continue_to(1, c, 1).expect("state 1 fits in u16");
    }
    b.accept(1, 42);
    let dfa = b.build().expect("empty pattern set must succeed");
    let src = preprocess_c_host(HELLO);
    let bytes = src.as_bytes();
    let len = bytes.len().min(u32::MAX as usize) as u32;
    let n1 = count_chunked_valid_tokens(
        &dfa.transitions,
        &dfa.token_ids,
        bytes,
        len,
        dfa.num_states,
        64,
        dfa.num_classes,
    );
    let n2 = count_chunked_valid_tokens(
        &dfa.transitions,
        &dfa.token_ids,
        bytes,
        len,
        dfa.num_states,
        64,
        dfa.num_classes,
    );
    assert_eq!(n1, n2);
    assert!(n1 > 0);
}
