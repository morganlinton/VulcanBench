//! Failure-oriented adversarial tests for NFA primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "nfa")]

use vyre_primitives::nfa::subgroup_nfa::*;

#[test]
fn cpu_step_empty_state_stays_empty() {
    let state = vec![0u32; LANES_PER_SUBGROUP];
    let trans = vec![0u32; 2 * 256 * LANES_PER_SUBGROUP];
    let eps = vec![0u32; 2 * LANES_PER_SUBGROUP];
    let out = cpu_step(&state, b'a', &trans, &eps, 2);
    assert_eq!(out, vec![0u32; LANES_PER_SUBGROUP]);
}

#[test]
fn cpu_step_max_states_boundary() {
    let num_states = MAX_STATES_PER_SUBGROUP;
    let state = vec![0u32; LANES_PER_SUBGROUP];
    let trans = vec![0u32; num_states * 256 * LANES_PER_SUBGROUP];
    let eps = vec![0u32; num_states * LANES_PER_SUBGROUP];
    let out = cpu_step(&state, 0, &trans, &eps, num_states);
    assert_eq!(out, vec![0u32; LANES_PER_SUBGROUP]);
}

#[test]
fn cpu_step_single_transition() {
    let num_states = 2;
    let mut trans = vec![0u32; num_states * 256 * LANES_PER_SUBGROUP];
    // state 0 on byte 'a' goes to state 1
    trans[(b'a' as usize) * LANES_PER_SUBGROUP] |= 1 << 1;
    let eps = vec![0u32; num_states * LANES_PER_SUBGROUP];
    let mut state = vec![0u32; LANES_PER_SUBGROUP];
    state[0] |= 1; // state 0 active
    let out = cpu_step(&state, b'a', &trans, &eps, num_states);
    assert_eq!(out[0] & (1 << 1), 1 << 1);
}

#[test]
fn cpu_step_epsilon_closure_propagates() {
    let num_states = 3;
    let mut trans = vec![0u32; num_states * 256 * LANES_PER_SUBGROUP];
    trans[(b'a' as usize) * LANES_PER_SUBGROUP] |= 1 << 1;
    let mut eps = vec![0u32; num_states * LANES_PER_SUBGROUP];
    eps[LANES_PER_SUBGROUP] |= 1 << 2;
    let mut state = vec![0u32; LANES_PER_SUBGROUP];
    state[0] |= 1;
    let out = cpu_step(&state, b'a', &trans, &eps, num_states);
    assert_eq!(out[0] & (1 << 1), 1 << 1);
    assert_eq!(out[0] & (1 << 2), 1 << 2);
}

#[test]
fn cpu_step_epsilon_closure_cycle_terminates() {
    let num_states = 2;
    let trans = vec![0u32; num_states * 256 * LANES_PER_SUBGROUP];
    let mut eps = vec![0u32; num_states * LANES_PER_SUBGROUP];
    // state 1 epsilon -> state 1 (self-loop)
    eps[LANES_PER_SUBGROUP] |= 1 << 1;
    let mut state = vec![0u32; LANES_PER_SUBGROUP];
    state[0] |= 1; // state 0
    let out = cpu_step(&state, b'x', &trans, &eps, num_states);
    // No transition on 'x', so only epsilon from state 0 (none)
    assert_eq!(out, vec![0u32; LANES_PER_SUBGROUP]);
}

#[test]
fn cpu_step_mismatched_state_length_panics() {
    let result = std::panic::catch_unwind(|| {
        let state = vec![0u32; LANES_PER_SUBGROUP - 1];
        let trans = vec![0u32; 2 * 256 * LANES_PER_SUBGROUP];
        let eps = vec![0u32; 2 * LANES_PER_SUBGROUP];
        cpu_step(&state, b'a', &trans, &eps, 2)
    });
    let payload = result.expect_err("mismatched state length must panic");
    assert!(payload.downcast_ref::<&str>().is_some() || payload.downcast_ref::<String>().is_some());
}

#[test]
fn cpu_step_mismatched_transition_length_panics() {
    let result = std::panic::catch_unwind(|| {
        let state = vec![0u32; LANES_PER_SUBGROUP];
        let trans = vec![0u32; 2 * 256 * LANES_PER_SUBGROUP - 1];
        let eps = vec![0u32; 2 * LANES_PER_SUBGROUP];
        cpu_step(&state, b'a', &trans, &eps, 2)
    });
    let payload = result.expect_err("mismatched transition length must panic");
    assert!(payload.downcast_ref::<&str>().is_some() || payload.downcast_ref::<String>().is_some());
}

#[test]
fn cpu_step_mismatched_epsilon_length_panics() {
    let result = std::panic::catch_unwind(|| {
        let state = vec![0u32; LANES_PER_SUBGROUP];
        let trans = vec![0u32; 2 * 256 * LANES_PER_SUBGROUP];
        let eps = vec![0u32; 2 * LANES_PER_SUBGROUP - 1];
        cpu_step(&state, b'a', &trans, &eps, 2)
    });
    let payload = result.expect_err("mismatched epsilon length must panic");
    assert!(payload.downcast_ref::<&str>().is_some() || payload.downcast_ref::<String>().is_some());
}

#[test]
fn nfa_step_program_max_states_accepted() {
    let _ = nfa_step("s", "i", "t", "e", "o", MAX_STATES_PER_SUBGROUP as u32);
}

#[test]
fn nfa_step_program_over_max_emits_trap_program() {
    let program = nfa_step("s", "i", "t", "e", "o", MAX_STATES_PER_SUBGROUP as u32 + 1);
    assert!(program.stats().trap());
}
