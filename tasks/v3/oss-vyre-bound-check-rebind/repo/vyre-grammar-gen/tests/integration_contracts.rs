//! Integration contracts for generated grammar wire blobs.

use vyre_grammar_gen::{decode_dfa_from_bytes, PackedBlob};

#[test]
fn integration_contract_round_trips_generated_dfa_blob() {
    let dfa = vyre_grammar_gen::build_c11_lexer_dfa();
    let blob = PackedBlob::from_dfa(&dfa);
    let decoded = decode_dfa_from_bytes(&blob.bytes)
        .expect("integration generator contract must decode DFA");
    assert_eq!(decoded.num_states, dfa.num_states);
    assert_eq!(decoded.num_classes, dfa.num_classes);
}
