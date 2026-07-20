//! Golden blake3 over [`lex_c11_max_munch_kinds`] for `corpus/hello.c` (preprocessed).
//!
//! Refresh: `cargo test -p vyre-grammar-gen --test gen_lex_hash -- --ignored --nocapture`

use vyre_grammar_gen::host_preprocess::preprocess_c_host;
use vyre_grammar_gen::kinds_blake3;
use vyre_grammar_gen::lex_c11_max_munch_kinds;

#[test]
fn hello_c_preproc_max_munch_kinds_hash() {
    let src = preprocess_c_host(include_str!("../corpus/hello.c"));
    let kinds = lex_c11_max_munch_kinds(src.as_bytes()).expect("lex");
    let h = kinds_blake3(&kinds);
    assert_eq!(
        h.to_hex().as_str(),
        include_str!("goldens/hello_max_munch_kinds.blake3").trim()
    );
}
