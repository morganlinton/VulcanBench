//! Property contracts for generated grammar table round trips.

use vyre_grammar_gen::{decode_dfa_from_bytes, PackedBlob};

#[test]
fn property_contract_blob_round_trip_preserves_transition_count() {
    let dfa = vyre_grammar_gen::build_c11_lexer_dfa();
    let blob = PackedBlob::from_dfa(&dfa);
    let decoded = decode_dfa_from_bytes(&blob.bytes)
        .expect("property generator contract must decode DFA");
    assert_eq!(decoded.transitions.len(), dfa.transitions.len());
}
