//! Conformance contracts for generated grammar blob headers.

use vyre_grammar_gen::{decode_dfa_from_bytes, PackedBlob};

#[test]
fn conformance_contract_generated_blob_has_stable_magic() {
    let dfa = vyre_grammar_gen::build_c11_lexer_dfa();
    let blob = PackedBlob::from_dfa(&dfa);
    assert!(blob.bytes.starts_with(b"SGGC"));
    assert!(decode_dfa_from_bytes(&blob.bytes).is_ok());
}
