//! Subgroup-cooperative NFA step primitive (G1).
//!
//! # What this is
//!
//! A Thompson-style NFA simulator where one subgroup simulates
//! up to `MAX_STATES_PER_SUBGROUP` states of a single NFA window.
//! Each lane owns a single `u32` holding 32 bits of the active
//! state-set. The per-byte step is:
//!
//! ```text
//!   1. Load transition_mask for (input_byte, this_lane_slot).
//!   2. Shuffle across the subgroup to gather every lane's active
//!      states that should reach this lane (via the NFA's
//!      transition table).
//!   3. Apply epsilon-closure: iterate `MAX_EPSILON_ITERS` times,
//!      OR'ing ε-reachable states. OR is idempotent so running to
//!      the fixed bound is equivalent to running to fixpoint; the
//!      cap guards against pathological inputs.
//!   4. Write the new state bitset back to DRAM.
//! ```
//!
//! # Encoding
//!
//! All NFAs in this module share one canonical encoding:
//!
//! - `state_bits` (per-lane u32): active-state bitset. Bit `i` in
//!   lane `k` means state `(k * 32 + i)` is active.
//! - `transition_buf` (ReadOnly, u32): lane-major
//!   `[num_states × 256 × LANES_PER_SUBGROUP]`. Entry
//!   `transition[src_state * 256 * LANES + byte * LANES + lane]`
//!   is a u32 holding the destination-state bits *this lane is
//!   responsible for* that state `src_state` reaches on byte
//!   `byte`.
//! - `epsilon_buf` (ReadOnly, u32): lane-major
//!   `[num_states × LANES_PER_SUBGROUP]`.
//!
//! Compact and cache-friendly. Higher-level NFA compositions emit this
//! canonical transition-table shape and handle tiling policy.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::nfa::subgroup_nfa_step";

/// Maximum NFA window width simulated by one subgroup.
/// `LANES_PER_SUBGROUP × 32 bits = 1024` states per subgroup.
/// For larger NFAs the composition layer tiles into 1024-state
/// windows.
pub const MAX_STATES_PER_SUBGROUP: usize = 1024;

/// Epsilon-closure iteration cap. Closures converge in the
/// diameter of the ε-graph (typically <8 for real NFAs). We cap
/// at MAX_STATES_PER_SUBGROUP so pathological inputs terminate;
/// because the closure is idempotent OR, extra iterations after
/// fixpoint are no-ops.
pub const MAX_EPSILON_ITERS: u32 = MAX_STATES_PER_SUBGROUP as u32;

/// How many u32 lanes encode the state set. Default subgroup width = 32.
/// Multi-subgroup tiling is the composition layer's job.
pub const LANES_PER_SUBGROUP: usize = 32;

/// Canonical NFA state-set buffer name.
pub const NAME_STATE: &str = "nfa_state";
/// Canonical input-byte buffer name.
pub const NAME_INPUT: &str = "nfa_input";
/// Canonical transition-table buffer name.
pub const NAME_TRANSITION: &str = "nfa_transition";
/// Canonical epsilon-closure table buffer name.
pub const NAME_EPSILON: &str = "nfa_epsilon";
/// Canonical output state-set buffer name.
pub const NAME_OUT: &str = "nfa_out_state";

/// Build a `Program` that performs one full NFA step for one
/// byte: transition + epsilon closure. One invocation per
/// state-lane; workgroup is a full NFA window.
///
/// See module docs for the buffer layout.
#[must_use]
pub fn nfa_step(
    state_buf: &str,
    input_buf: &str,
    transition_buf: &str,
    epsilon_buf: &str,
    out_buf: &str,
    num_states: u32,
) -> Program {
    if num_states as usize > MAX_STATES_PER_SUBGROUP {
        return crate::invalid_output_program(
            OP_ID,
            out_buf,
            DataType::U32,
            format!("Fix: num_states {num_states} exceeds MAX_STATES_PER_SUBGROUP={MAX_STATES_PER_SUBGROUP}; caller must tile at the composition layer."),
        );
    }

    let lane = Expr::InvocationId { axis: 0 };
    let lane_u32 = || lane.clone();

    let mut body: Vec<Node> = Vec::new();

    // cur = state[lane]
    body.push(Node::let_bind("cur", Expr::load(state_buf, lane_u32())));

    // byte = input[0] & 0xff
    body.push(Node::let_bind(
        "byte",
        Expr::bitand(Expr::load(input_buf, Expr::u32(0)), Expr::u32(0xff)),
    ));

    // acc = 0  (accumulator for this lane's destination bits)
    body.push(Node::let_bind("acc", Expr::u32(0)));

    // Transition gather. For each peer lane k and each bit i the
    // peer has set for state (k*32 + i), OR in this lane's slice
    // of the transition table row.
    //
    // We unroll over k (LANES_PER_SUBGROUP peer lanes) because
    // subgroup_shuffle requires a compile-time peer constant in the
    // target-text lowering. Inner bit loop is also unrolled so each byte
    // step is a predictable straight-line block of ops.
    for k in 0..LANES_PER_SUBGROUP as u32 {
        body.push(Node::let_bind(
            "peer",
            Expr::subgroup_shuffle(Expr::var("cur"), Expr::u32(k)),
        ));
        for i in 0..32_u32 {
            let src_state = k * 32 + i;
            if src_state >= num_states {
                continue;
            }
            // if ((peer >> i) & 1) != 0: acc |= transition[src_row + byte*LANES + lane]
            let src_row = src_state * 256 * LANES_PER_SUBGROUP as u32;
            body.push(Node::if_then(
                Expr::ne(
                    Expr::bitand(Expr::shr(Expr::var("peer"), Expr::u32(i)), Expr::u32(1)),
                    Expr::u32(0),
                ),
                vec![Node::assign(
                    "acc",
                    Expr::bitor(
                        Expr::var("acc"),
                        Expr::load(
                            transition_buf,
                            Expr::add(
                                Expr::add(
                                    Expr::u32(src_row),
                                    Expr::mul(
                                        Expr::var("byte"),
                                        Expr::u32(LANES_PER_SUBGROUP as u32),
                                    ),
                                ),
                                lane_u32(),
                            ),
                        ),
                    ),
                )],
            ));
        }
    }

    // Epsilon closure  -  bounded loop. Because OR is idempotent,
    // running a fixed count ≥ ε-diameter reaches fixpoint.
    // `loop_for` emits a counted `for var in 0..N` loop that the
    // optimizer can unroll when N is small (common for real NFAs).
    let mut eps_body: Vec<Node> = Vec::new();
    for k in 0..LANES_PER_SUBGROUP as u32 {
        eps_body.push(Node::let_bind(
            "peer",
            Expr::subgroup_shuffle(Expr::var("acc"), Expr::u32(k)),
        ));
        for i in 0..32_u32 {
            let src_state = k * 32 + i;
            if src_state >= num_states {
                continue;
            }
            eps_body.push(Node::if_then(
                Expr::ne(
                    Expr::bitand(Expr::shr(Expr::var("peer"), Expr::u32(i)), Expr::u32(1)),
                    Expr::u32(0),
                ),
                vec![Node::assign(
                    "acc",
                    Expr::bitor(
                        Expr::var("acc"),
                        Expr::load(
                            epsilon_buf,
                            Expr::add(
                                Expr::mul(
                                    Expr::u32(src_state),
                                    Expr::u32(LANES_PER_SUBGROUP as u32),
                                ),
                                lane_u32(),
                            ),
                        ),
                    ),
                )],
            ));
        }
    }

    // Choose a safe static cap for the emitted loop. For very small
    // NFAs we cap at num_states (the diameter upper bound); for
    // anything bigger we cap at a conservative 32. Callers that need
    // a higher epsilon-closure bound compose multiple subgroup steps.
    let eps_iters = num_states.clamp(1, 32);
    body.push(Node::loop_for(
        "eps_iter",
        Expr::u32(0),
        Expr::u32(eps_iters),
        eps_body,
    ));

    // out[lane] = acc
    body.push(Node::store(out_buf, lane_u32(), Expr::var("acc")));

    Program::wrapped(
        vec![
            BufferDecl::storage(state_buf, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(LANES_PER_SUBGROUP as u32),
            BufferDecl::storage(input_buf, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(transition_buf, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(num_states * 256 * LANES_PER_SUBGROUP as u32),
            BufferDecl::storage(epsilon_buf, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(num_states * LANES_PER_SUBGROUP as u32),
            BufferDecl::storage(out_buf, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(LANES_PER_SUBGROUP as u32),
        ],
        [LANES_PER_SUBGROUP as u32, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(lane_u32(), Expr::u32(LANES_PER_SUBGROUP as u32)),
                body,
            )]),
        }],
    )
}

/// CPU-reference NFA step. Runs the epsilon closure to fixpoint
/// (not fixed iterations) so tests prove the fixpoint semantics
/// without depending on the GPU's iteration cap.
///
/// `state`: active-state bitset of length `LANES_PER_SUBGROUP`.
/// `byte`: input byte [0, 256).
/// `transition`: lane-major `[num_states × 256 × LANES]`.
/// `epsilon`: lane-major `[num_states × LANES]`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_step(
    state: &[u32],
    byte: u8,
    transition: &[u32],
    epsilon: &[u32],
    num_states: usize,
) -> Vec<u32> {
    let mut acc = Vec::new();
    let mut scratch = Vec::new();
    cpu_step_into(
        state,
        byte,
        transition,
        epsilon,
        num_states,
        &mut acc,
        &mut scratch,
    );
    acc
}

/// CPU-reference NFA step using caller-owned buffers.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_step_into(
    state: &[u32],
    byte: u8,
    transition: &[u32],
    epsilon: &[u32],
    num_states: usize,
    acc: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
) {
    acc.clear();
    acc.resize(LANES_PER_SUBGROUP, 0);
    scratch.clear();
    scratch.resize(LANES_PER_SUBGROUP, 0);

    assert!(
        num_states <= MAX_STATES_PER_SUBGROUP,
        "subgroup NFA CPU oracle received num_states={num_states} above MAX_STATES_PER_SUBGROUP={MAX_STATES_PER_SUBGROUP}. Fix: tile the NFA before parity comparison."
    );
    assert_eq!(
        state.len(),
        LANES_PER_SUBGROUP,
        "subgroup NFA CPU oracle received state_len={} but requires LANES_PER_SUBGROUP={LANES_PER_SUBGROUP}. Fix: pass a complete subgroup state bitset.",
        state.len()
    );
    let expected_transition = num_states
        .checked_mul(256 * LANES_PER_SUBGROUP)
        .unwrap_or_else(|| {
            panic!(
                "subgroup NFA CPU oracle num_states={num_states} overflows transition table length. Fix: tile the NFA before parity comparison."
            )
        });
    let expected_epsilon = num_states.checked_mul(LANES_PER_SUBGROUP).unwrap_or_else(|| {
        panic!(
            "subgroup NFA CPU oracle num_states={num_states} overflows epsilon table length. Fix: tile the NFA before parity comparison."
        )
    });
    assert_eq!(
        transition.len(),
        expected_transition,
        "subgroup NFA CPU oracle received transition_len={} but requires {expected_transition}. Fix: pass a complete num_states * 256 * LANES transition table.",
        transition.len()
    );
    assert_eq!(
        epsilon.len(),
        expected_epsilon,
        "subgroup NFA CPU oracle received epsilon_len={} but requires {expected_epsilon}. Fix: pass a complete num_states * LANES epsilon table.",
        epsilon.len()
    );

    for (k, &peer) in state.iter().enumerate() {
        for i in 0..32 {
            let src_state = k * 32 + i;
            if src_state >= num_states {
                break;
            }
            if (peer >> i) & 1 == 0 {
                continue;
            }
            for (lane, slot) in acc.iter_mut().enumerate() {
                let idx = src_state * 256 * LANES_PER_SUBGROUP
                    + (byte as usize) * LANES_PER_SUBGROUP
                    + lane;
                *slot |= transition[idx];
            }
        }
    }

    // Epsilon closure  -  real fixpoint. OR-idempotent so termination
    // is guaranteed; cap defends against caller-supplied malformed
    // tables.
    for _ in 0..MAX_EPSILON_ITERS as usize {
        scratch.copy_from_slice(acc);
        for (k, &peer) in scratch.iter().enumerate() {
            for i in 0..32 {
                let src_state = k * 32 + i;
                if src_state >= num_states {
                    break;
                }
                if (peer >> i) & 1 == 0 {
                    continue;
                }
                for (lane, slot) in acc.iter_mut().enumerate() {
                    let idx = src_state * LANES_PER_SUBGROUP + lane;
                    *slot |= epsilon[idx];
                }
            }
        }
        if acc == scratch {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_lane_row(targets: &[usize]) -> [u32; LANES_PER_SUBGROUP] {
        let mut row = [0_u32; LANES_PER_SUBGROUP];
        for &t in targets {
            assert!(t < MAX_STATES_PER_SUBGROUP);
            row[t / 32] |= 1 << (t % 32);
        }
        row
    }

    fn build_transition(edges: &[(usize, u8, Vec<usize>)], num_states: usize) -> Vec<u32> {
        let mut tbl = vec![0_u32; num_states * 256 * LANES_PER_SUBGROUP];
        for (src, byte, targets) in edges {
            let row = pack_lane_row(targets);
            for (lane, word) in row.iter().enumerate() {
                let idx =
                    src * 256 * LANES_PER_SUBGROUP + (*byte as usize) * LANES_PER_SUBGROUP + lane;
                tbl[idx] |= *word;
            }
        }
        tbl
    }

    fn build_epsilon(edges: &[(usize, Vec<usize>)], num_states: usize) -> Vec<u32> {
        let mut tbl = vec![0_u32; num_states * LANES_PER_SUBGROUP];
        for (src, targets) in edges {
            let row = pack_lane_row(targets);
            for lane in 0..LANES_PER_SUBGROUP {
                tbl[src * LANES_PER_SUBGROUP + lane] |= row[lane];
            }
        }
        tbl
    }

    fn seed_state(bits: &[usize]) -> Vec<u32> {
        pack_lane_row(bits).to_vec()
    }

    #[test]
    fn single_transition() {
        let trans = build_transition(&[(0, b'a', vec![1])], 2);
        let eps = build_epsilon(&[], 2);
        let out = cpu_step(&seed_state(&[0]), b'a', &trans, &eps, 2);
        assert_eq!(out, seed_state(&[1]));
    }

    #[test]
    fn no_transition_on_wrong_byte() {
        let trans = build_transition(&[(0, b'a', vec![1])], 2);
        let eps = build_epsilon(&[], 2);
        let out = cpu_step(&seed_state(&[0]), b'b', &trans, &eps, 2);
        assert_eq!(out, vec![0_u32; LANES_PER_SUBGROUP]);
    }

    #[test]
    fn epsilon_closure_applies() {
        let trans = build_transition(&[(0, b'a', vec![1])], 3);
        let eps = build_epsilon(&[(1, vec![2])], 3);
        let out = cpu_step(&seed_state(&[0]), b'a', &trans, &eps, 3);
        assert_eq!(out, seed_state(&[1, 2]));
    }

    #[test]
    fn epsilon_closure_transitive() {
        let trans = build_transition(&[(0, b'a', vec![1])], 4);
        let eps = build_epsilon(&[(1, vec![2]), (2, vec![3])], 4);
        let out = cpu_step(&seed_state(&[0]), b'a', &trans, &eps, 4);
        assert_eq!(out, seed_state(&[1, 2, 3]));
    }

    #[test]
    fn multiple_sources_union() {
        let trans = build_transition(&[(0, b'a', vec![1]), (2, b'a', vec![3])], 4);
        let eps = build_epsilon(&[], 4);
        let out = cpu_step(&seed_state(&[0, 2]), b'a', &trans, &eps, 4);
        assert_eq!(out, seed_state(&[1, 3]));
    }

    #[test]

    fn epsilon_fanout() {
        let trans = build_transition(&[(0, b'a', vec![1])], 5);
        let eps = build_epsilon(&[(1, vec![2, 3, 4])], 5);
        let out = cpu_step(&seed_state(&[0]), b'a', &trans, &eps, 5);
        assert_eq!(out, seed_state(&[1, 2, 3, 4]));
    }

    #[test]
    fn empty_state_stays_empty() {
        let trans = build_transition(&[(0, b'a', vec![1])], 2);
        let eps = build_epsilon(&[(1, vec![0])], 2);
        let out = cpu_step(&[0; LANES_PER_SUBGROUP], b'a', &trans, &eps, 2);
        assert_eq!(out, vec![0_u32; LANES_PER_SUBGROUP]);
    }

    #[test]
    fn self_epsilon_loop_terminates() {
        let trans = build_transition(&[(0, b'a', vec![1])], 2);
        let eps = build_epsilon(&[(1, vec![1])], 2);
        let out = cpu_step(&seed_state(&[0]), b'a', &trans, &eps, 2);
        assert_eq!(out, seed_state(&[1]));
    }

    #[test]
    fn cross_lane_state_simulated_correctly() {
        let trans = build_transition(&[(0, b'a', vec![35])], 36);
        let eps = build_epsilon(&[], 36);
        let out = cpu_step(&seed_state(&[0]), b'a', &trans, &eps, 36);
        let mut expected = vec![0_u32; LANES_PER_SUBGROUP];
        expected[1] = 1 << 3;
        assert_eq!(out, expected);
    }

    #[test]
    fn cpu_step_into_reuses_buffers_and_rejects_malformed_tables() {
        let trans = build_transition(&[(0, b'a', vec![1])], 2);
        let eps = build_epsilon(&[], 2);
        let mut acc = Vec::with_capacity(LANES_PER_SUBGROUP + 8);
        let acc_ptr = acc.as_ptr();
        let mut scratch = Vec::with_capacity(LANES_PER_SUBGROUP + 8);
        let scratch_ptr = scratch.as_ptr();

        cpu_step_into(
            &seed_state(&[0]),
            b'a',
            &trans,
            &eps,
            2,
            &mut acc,
            &mut scratch,
        );

        assert_eq!(acc.as_ptr(), acc_ptr);
        assert_eq!(scratch.as_ptr(), scratch_ptr);
        assert_eq!(acc, seed_state(&[1]));

        let malformed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cpu_step_into(&[1], b'a', &trans, &eps, 2, &mut acc, &mut scratch);
        }));
        malformed.expect_err("cpu_step_into with wrong state length must panic");
    }

    #[test]
    fn num_states_bound_enforced_at_max() {
        let program = nfa_step("s", "i", "t", "e", "o", 1024);
        assert_eq!(
            program.buffers().len(),
            5,
            "max-bound NFA step must declare state/input/transition/epsilon/output buffers"
        );
    }

    #[test]
    fn num_states_over_bound_traps() {
        let p = nfa_step("s", "i", "t", "e", "o", 1025);
        assert!(p.stats().trap());
    }

    #[test]
    fn emitted_program_declares_expected_buffers() {
        let p = nfa_step("s", "i", "t", "e", "o", 4);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["s", "i", "t", "e", "o"]);
        // Counts scale with num_states.
        let find = |name: &str| p.buffers.iter().find(|b| b.name() == name).unwrap();
        assert_eq!(find("s").count, LANES_PER_SUBGROUP as u32);
        assert_eq!(find("t").count, 4 * 256 * LANES_PER_SUBGROUP as u32);
        assert_eq!(find("e").count, 4 * LANES_PER_SUBGROUP as u32);
    }

    #[test]
    fn emitted_program_uses_subgroup_workgroup() {
        let p = nfa_step("s", "i", "t", "e", "o", 4);
        assert_eq!(p.workgroup_size, [LANES_PER_SUBGROUP as u32, 1, 1]);
    }
}
