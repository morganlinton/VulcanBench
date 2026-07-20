//! Unit contracts for generated C parser grammar tables.

use vyre_grammar_gen::{build_c11_lexer_dfa, validate_lr_table, LrBuilder};

#[test]
fn unit_contract_generates_c11_lexer_and_validates_lr_table() {
    let dfa = build_c11_lexer_dfa();
    assert!(dfa.num_states > 1, "unit generator contract needs a non-empty DFA");

    let mut builder = LrBuilder::new(1, 1, 1);
    let prod = builder.add_production(0, 0);
    builder.set_action(0, 0, vyre_grammar_gen::lr::Action::Reduce(prod));
    let table = builder.build();
    validate_lr_table(&table).expect("unit LR table contract must validate");
}
