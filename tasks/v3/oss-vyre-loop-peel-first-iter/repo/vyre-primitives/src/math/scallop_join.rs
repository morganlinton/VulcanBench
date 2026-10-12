//! `scallop_join`  -  Scallop-style probabilistic Datalog join, GPU-resident.
//!
//! Compiles a Datalog fixpoint into GPU-resident dispatch phases by
//! emitting a Lineage-semiring relational join. Small matrices use a
//! block-local convergence loop; large matrices expose split-visible
//! GridSync phases for multi-block CUDA dispatch. The output cell
//! `C[i,j]` is the bitset union of clauses
//! participating in any `i ⇝ j` derivation through one join step.
//!
//! # Why ship this as a named primitive instead of "compose them yourself"
//!
//! Two reasons:
//!
//! ## (a) The fixpoint contract
//!
//! Datalog fixpoint converges when no new fact is derived. Under the
//! Lineage semiring that means no clause-bitset OR'd into any cell
//! flips a 0 bit to 1  -  the canonical convergence signal `next ==
//! current` per word. `scallop_join` packages the Lineage transfer and
//! convergence loop together so callers do not re-derive that the
//! Lineage semiring's monotonic OR-accumulator is safe with ping-pong
//! equality convergence. Other semirings would NOT be safe  -  `MinPlus`
//! accumulators decrease over iterations, which the equality check would
//! treat as "changed = 1" indefinitely until the absolute minimum settles.
//! So the recursion-thesis-clean wrapper is the contract:
//!
//! > "scallop_join is exactly the Datalog-shaped, monotone,
//! >  GPU-resident Lineage fixpoint."
//!
//! ## (b) Two consumers, recursion thesis closed from day 1
//!
//! - **User dialect consumer**: probabilistic Datalog programs (Scallop
//!   programs compile each rule body to one `scallop_join`). Substrate
//!   for neuro-symbolic reasoning systems.
//! - **vyre-self consumer**: rule-provenance tracking for external analyzer / any
//!   substrate that needs to ask "which input rule produced this output
//!   finding?" The answer is a Datalog query over (rule_id, derives,
//!   finding_id), and `scallop_join` is the GPU-resident execution.
//!   See [`crate::math::scallop_join::PROVENANCE_SELF_CONSUMER`].
//!
//! # Algorithm
//!
//! ```text
//! initial:    R[0]   = adjacency matrix encoding source → target
//!                      facts; cell is the bitset of clauses introducing
//!                      that edge (Lineage encoding).
//! transfer:   R[t+1] = R[t] ⊗_Lineage A_join,  where A_join is the
//!                      static join-rule adjacency. Combine = "OR
//!                      participating clauses across one path step",
//!                      Accumulate = "OR alternative derivations into
//!                      the same cell."
//! converge:   stop when R[t+1] == R[t] per cell.
//! ```
//!
//! Each cell is a single u32 bitset of clauses (capacity 32). Multi-word
//! lineage belongs in a distinct `scallop_join_wide` op so larger clause
//! sets have their own schema; this primitive is the canonical
//! single-word version with the contract test that distinguishes "no
//! edge" from "edge with empty clause set" via the zero-absorbing
//! combine.
//!
//! # Wiring contract
//!
//! Caller supplies:
//!
//! - `state`: `n × n` cell buffer (ReadWrite). Initialized by caller
//!   with the seed facts; mutated to fixpoint by the dispatch.
//! - `next`: `n × n` scratch buffer (ReadWrite). Reused as the
//!   ping-pong target between fixpoint iterations.
//! - `join_rules`: `n × n` static join-rule adjacency (ReadOnly).
//!   `join_rules[i,j]` is the clause bitset that, when present at
//!   `state[i,k]` and `join_rules[k,j]` for some k, derives a fact at
//!   `state[i,j]`.
//! - `changed`: 1-word convergence flag (ReadWrite, atomic OR).
//! - `n`: matrix dimension (relations encoded as n × n cells).
//! - `max_iterations`: hard upper bound (Datalog fixpoint is monotone
//!   so converges in ≤ n^2 iterations; cap at a safety multiple).
//!
//! # CPU reference
//!
//! [`crate::math::scallop_join::cpu_ref`] performs the same fixpoint iteration on host arrays and
//! is the parity oracle for every GPU dispatch.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Node, Program};

use crate::math::scallop_persistent::{
    ceil_div_u32, single_word_lineage_body, single_word_lineage_grid_sync_body,
};
#[cfg(any(test, feature = "cpu-parity"))]
use crate::math::semiring_gemm::{semiring_gemm_cpu_into, Semiring};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::scallop_join";
/// One lane per relation cell in the single-word lineage fixpoint.
pub const SCALLOP_JOIN_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for the Scallop kernel.
#[must_use]
pub const fn scallop_join_dispatch_grid(_n: u32) -> [u32; 3] {
    let cells = _n.saturating_mul(_n);
    let blocks = ceil_div_u32(cells, SCALLOP_JOIN_WORKGROUP_SIZE[0]);
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Documentation hook for the recursion-thesis self-consumer wired in
/// `vyre-libs::self_substrate::scallop_provenance`. Updates to this
/// constant must update the self-consumer module's doc-link.
pub const PROVENANCE_SELF_CONSUMER: &str = "vyre-libs::self_substrate::scallop_provenance";

/// Build a fused Datalog-fixpoint Program: iterate the Lineage join until
/// convergence for block-local matrices, or through fixed split-visible phases
/// for larger matrices.
///
/// The transfer step writes `next` from `state` and the supplied
/// join-rule matrix, then compares and copies the ping-pong buffer. Small
/// matrices finish inside one workgroup. Larger matrices surface top-level
/// GridSync barriers so host dispatch can split transfer and compare phases
/// across blocks.
///
/// # Panics
///
/// Panics if `n == 0` or `max_iterations == 0`.
#[must_use]
pub fn scallop_join(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    max_iterations: u32,
) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            state,
            DataType::U32,
            format!("Fix: scallop_join requires n > 0, got {n}."),
        );
    }
    if max_iterations == 0 {
        return crate::invalid_output_program(
            OP_ID,
            state,
            DataType::U32,
            "Fix: scallop_join requires max_iterations > 0, got 0.".to_string(),
        );
    }

    // n*n cells, each one u32  -  one "word" per cell for ping-pong.
    let words = n.saturating_mul(n);

    let body = if words <= SCALLOP_JOIN_WORKGROUP_SIZE[0] {
        single_word_lineage_body(
            state,
            next,
            join_rules,
            changed,
            n,
            words,
            max_iterations,
            SCALLOP_JOIN_WORKGROUP_SIZE[0],
        )
    } else {
        single_word_lineage_grid_sync_body(
            state,
            next,
            join_rules,
            changed,
            n,
            words,
            max_iterations,
        )
    };

    // Rebuild the Program with both the fixpoint trio and the
    // additional join_rules ReadOnly buffer surfaced.
    let entry: Vec<Node> = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(body),
    }];

    Program::wrapped(
        vec![
            BufferDecl::storage(state, 0, BufferAccess::ReadWrite, DataType::U32).with_count(words),
            BufferDecl::storage(next, 1, BufferAccess::ReadWrite, DataType::U32).with_count(words),
            BufferDecl::storage(changed, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(join_rules, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
        ],
        SCALLOP_JOIN_WORKGROUP_SIZE,
        entry,
    )
}

/// CPU reference. Iterates `state ← semiring_gemm_cpu(state, join_rules,
/// Lineage)` until the result no longer changes or `max_iterations` is
/// reached. Returns `(final_state, iterations_run)`.
///
/// The Datalog fixpoint is monotone under Lineage (combine + accumulate
/// are both OR-of-bitset, which only sets bits, never clears them), so
/// it converges in at most `n^2` iterations. The `max_iterations` cap
/// is a defensive safety bound  -  a non-monotone caller (which would be
/// a contract violation) is detected and reported as the iteration
/// count returning the cap itself.
///
/// # Panics
///
/// Panics if `state.len() != n*n` or `join_rules.len() != n*n`.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn cpu_ref(state: &[u32], join_rules: &[u32], n: u32, max_iterations: u32) -> (Vec<u32>, u32) {
    let mut current = Vec::new();
    let mut next = Vec::new();
    let iters = cpu_ref_into(
        state,
        join_rules,
        n,
        max_iterations,
        &mut current,
        &mut next,
    );
    (current, iters)
}

/// CPU reference using caller-owned state and scratch buffers.
///
/// `current` is overwritten with the final fixpoint state. `next` is a
/// scratch GEMM target retained for reuse across calls.
///
/// # Panics
///
/// Panics if `state.len() != n*n` or `join_rules.len() != n*n`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> u32 {
    let cells = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_mul(n))
        .unwrap_or_else(|| {
            panic!(
                "scallop_join CPU oracle n={n} overflows relation matrix word count. Fix: shard the relation matrix before parity comparison."
            )
        });
    assert_eq!(
        state.len(),
        cells,
        "scallop_join CPU oracle received state_len={} for n={n}. Fix: pass a complete n*n state matrix before parity comparison.",
        state.len()
    );
    assert_eq!(
        join_rules.len(),
        cells,
        "scallop_join CPU oracle received join_rules_len={} for n={n}. Fix: pass a complete n*n rule matrix before parity comparison.",
        join_rules.len()
    );
    current.clear();
    current.extend_from_slice(state);
    for iter in 0..max_iterations {
        semiring_gemm_cpu_into(current, join_rules, n, n, n, Semiring::Lineage, next);
        // Datalog monotonicity: each iteration's output is a
        // bitwise-OR-superset of the input on every cell. Convergence
        // = no bit changed. Take the OR of current and next so the
        // initial seed facts persist across iterations (semiring_gemm
        // by itself replaces, not accumulates).
        let mut changed = false;
        for (cell, derived) in current.iter_mut().zip(next.iter()) {
            let merged = *cell | *derived;
            changed |= merged != *cell;
            *cell = merged;
        }
        if !changed {
            return iter;
        }
    }
    max_iterations
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || scallop_join("state", "next", "join_rules", "changed", 2, 4),
        Some(|| {
            // Seed: state[0,1] = clause-bit 0 (a derives b directly).
            // join: join_rules[1,1] = clause-bit 1 (b derives b through itself, transitively).
            // After one round: state[0,1] |= join_rules[1,1] applied through k=1 → bits 0 + 1.
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0b01, 0, 0]),
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0]),
                to_bytes(&[0, 0, 0, 0b10]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0b11, 0, 0]), // state
                to_bytes(&[0, 0b11, 0, 0]), // next
                to_bytes(&[0]),             // changed
            ]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;
    use vyre_foundation::MemoryOrdering;

    #[test]
    fn cpu_ref_one_step_join() {
        // 2x2 system. state[0,1]=clause 0; join_rules[1,1]=clause 1.
        // First fixpoint round: derive state[0,1] |= state[0,k] · join_rules[k,1]
        //   = state[0,1] · join_rules[1,1] = bit0 · bit1 (Lineage combine: OR
        //   when both nonzero) = bits 0+1.
        let state = vec![0u32, 0b01, 0u32, 0u32];
        let join_rules = vec![0u32, 0u32, 0u32, 0b10];
        let (final_state, iters) = cpu_ref(&state, &join_rules, 2, 16);
        // state[0,1] should now have bit 1 OR'd in (the lineage of the
        // newly derived path).
        assert_eq!(
            final_state[1] & 0b10,
            0b10,
            "Lineage of clause 1 must propagate to state[0,1] after one round"
        );
        // bit 0 (the seed) must persist  -  Datalog never retracts facts.
        assert_eq!(
            final_state[1] & 0b01,
            0b01,
            "seed clause 0 must persist through the fixpoint"
        );
        assert!(
            iters <= 4,
            "small system should converge quickly, got {iters}"
        );
    }

    #[test]
    fn cpu_ref_converges_on_idempotent_input() {
        // No new facts can be derived: state has only the diagonal
        // self-loop, join_rules has no clauses at all → first iteration
        // produces zeros + the seed; second iteration produces the same
        // → converges at iter 1.
        let state = vec![0b01, 0u32, 0u32, 0b01];
        let join_rules = vec![0u32; 4];
        let (final_state, iters) = cpu_ref(&state, &join_rules, 2, 16);
        assert_eq!(
            final_state, state,
            "idempotent system must not change state"
        );
        assert!(iters <= 2, "idempotent system converges in ≤ 2 iters");
    }

    #[test]
    fn cpu_ref_into_reuses_buffers() {
        let state = vec![0u32, 0b01, 0u32, 0u32];
        let join_rules = vec![0u32, 0u32, 0u32, 0b10];
        let mut current = Vec::with_capacity(128);
        let mut next = Vec::with_capacity(128);
        let current_ptr = current.as_ptr();
        let next_ptr = next.as_ptr();
        let iters = cpu_ref_into(&state, &join_rules, 2, 16, &mut current, &mut next);
        assert!(iters <= 4);
        assert_eq!(current[1] & 0b11, 0b11);
        assert_eq!(current.as_ptr(), current_ptr);
        assert_eq!(next.as_ptr(), next_ptr);
    }

    #[test]
    fn cpu_ref_transitive_closure() {
        // 3-cell chain: state[0,1]=bit0, state[1,2]=bit1.
        // join_rules: same as state (each path step adds its own bit).
        // Fixpoint should produce state[0,2] with both bits set.
        let mut state = vec![0u32; 9];
        state[0 * 3 + 1] = 0b001; // (0→1) clause 0
        state[1 * 3 + 2] = 0b010; // (1→2) clause 1
        let join_rules = state.clone();
        let (final_state, iters) = cpu_ref(&state, &join_rules, 3, 16);
        // Transitive derivation 0→1→2 must accumulate clauses 0 and 1.
        assert_eq!(
            final_state[0 * 3 + 2] & 0b011,
            0b011,
            "transitive 0→2 must collect lineage of both edges; got 0x{:x}",
            final_state[0 * 3 + 2]
        );
        assert!(iters <= 8, "3-node chain should converge fast");
    }

    #[test]
    fn cpu_ref_zero_absorbing_no_phantom_lineage() {
        // Edge present with empty clause set vs no edge  -  Lineage
        // combine is zero-absorbing, so an empty cell × any
        // join-rule cell stays zero (no spurious lineage).
        let state = vec![0u32; 4]; // no facts
        let join_rules = vec![0b11u32; 4];
        let (final_state, _) = cpu_ref(&state, &join_rules, 2, 16);
        assert_eq!(
            final_state, state,
            "no seed facts → no derivations regardless of rule set; \
             zero-absorbing combine prevents phantom lineage"
        );
    }

    #[test]
    fn program_declares_four_buffers() {
        let p = scallop_join("s", "n", "j", "c", 2, 4);
        let bufs = p.buffers();
        assert_eq!(bufs.len(), 4, "scallop_join must declare 4 buffers");
        assert_eq!(p.workgroup_size(), SCALLOP_JOIN_WORKGROUP_SIZE);
        let names: Vec<&str> = bufs.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"s"));
        assert!(names.contains(&"n"));
        assert!(names.contains(&"j"));
        assert!(names.contains(&"c"));
    }

    #[test]
    fn dispatch_grid_scales_large_relations_into_blocks() {
        assert_eq!(scallop_join_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(scallop_join_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(scallop_join_dispatch_grid(16), [1, 1, 1]);
        assert_eq!(scallop_join_dispatch_grid(17), [2, 1, 1]);
        assert_eq!(scallop_join_dispatch_grid(33), [5, 1, 1]);
    }

    #[test]
    fn large_program_uses_split_visible_grid_sync() {
        let p = scallop_join("s", "n", "j", "c", 17, 4);
        assert_eq!(count_grid_sync(p.entry()), 7);
    }

    #[test]
    fn rejects_zero_n_with_trap() {
        let p = scallop_join("s", "n", "j", "c", 0, 4);
        assert!(p.stats().trap());
    }

    #[test]
    fn rejects_zero_max_iterations_with_trap() {
        let p = scallop_join("s", "n", "j", "c", 2, 0);
        assert!(p.stats().trap());
    }

    fn count_grid_sync(nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|node| match node {
                Node::Barrier {
                    ordering: MemoryOrdering::GridSync,
                } => 1,
                Node::If {
                    then, otherwise, ..
                } => count_grid_sync(then) + count_grid_sync(otherwise),
                Node::Loop { body, .. } | Node::Block(body) => count_grid_sync(body),
                Node::Region { body, .. } => count_grid_sync(body),
                _ => 0,
            })
            .sum()
    }
}
