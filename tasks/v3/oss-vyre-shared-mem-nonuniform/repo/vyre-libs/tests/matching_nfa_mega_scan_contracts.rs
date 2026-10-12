//! Contracts for the subgroup-NFA mega-scan integrator.

#![cfg(feature = "matching-nfa")]
#![allow(deprecated)]
use vyre_foundation::match_result::Match;
use vyre_libs::scan::{dispatch_io, mega_scan, nfa};

#[test]
fn candidate_start_dispatch_uses_one_workgroup_per_byte() {
    let cfg = dispatch_io::candidate_start_dispatch_config(129);
    assert_eq!(cfg.grid_override, Some([129, 1, 1]));
}

#[test]
fn nfa_compile_records_terminal_state_ids() {
    let plan = nfa::compile(&["abc", "de"]);
    assert_eq!(plan.accept_states, vec![(0, 3), (1, 2)]);
    assert_eq!(plan.accept_state_ids, vec![3, 5]);
}

#[test]
fn nfa_scan_input_buffer_is_packed_bytes() {
    let program = nfa::nfa_scan(&["abc"], "input", "hits", 6);
    let input = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "input")
        .expect("input buffer");
    assert_eq!(input.count, 2);
}

#[test]
fn mega_reference_scan_reports_nonzero_start_offsets() {
    let pipe = mega_scan::build(&["abc", "bc"], "input", "hits", 4);
    let matches = pipe.reference_scan(b"zabc");
    assert!(matches.contains(&Match::new(0, 1, 4)));
    assert!(matches.contains(&Match::new(1, 2, 4)));
}
