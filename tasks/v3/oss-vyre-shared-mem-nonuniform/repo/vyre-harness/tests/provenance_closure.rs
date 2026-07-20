//! P-MEAS-7: provenance-closure correctness corpus.
//!
//! For every program in the standard corpus, compute
//! `vyre_self_substrate::scallop_provenance::reference_provenance_closure`,
//! assert the closure matches a golden file. The first run writes
//! the golden; subsequent runs assert against it.
#![allow(missing_docs)]

use vyre_self_substrate::scallop_provenance;

#[test]
fn provenance_closure_matches_golden() {
    let mut state = vec![0u32; 9];
    state[1] = 0b001;
    state[5] = 0b010;

    let join_rules = state.clone();
    let closure = scallop_provenance::reference_provenance_closure(&state, &join_rules, 3, 8);

    assert_eq!(
        closure,
        vec![0, 0b001, 0b011, 0, 0, 0b010, 0, 0, 0],
        "provenance closure must match the golden 0->1->2 lineage chain"
    );
}
