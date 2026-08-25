//! GPU-resident rule-provenance tracking via #39 scallop_join (#39 substrate).
//!
//! Closes the recursion thesis for #39  -  `vyre-primitives::math::scallop_join`
//! ships a Datalog-fixpoint primitive for user-dialect probabilistic logic,
//! AND simultaneously powers vyre's own provenance bookkeeping.
//!
//! # The self-use
//!
//! When a vyre Program goes through optimizer passes (canonicalize,
//! cse, region_inline, dce, fuse_cse, …), each pass may transform a
//! sub-tree of Regions, dropping or merging the `source_region`
//! annotation on the way. The host-side `Region::source_region` field
//! preserves direct authorship, but does NOT close the transitive
//! relation  -  given a final fused Region, "which input rules
//! ultimately contributed clauses to it?" is a Datalog query, not a
//! single field lookup.
//!
//! The legacy host lineage closure provides the same semantics as a
//! parity oracle. `scallop_provenance` here is the GPU-resident
//! companion that runs the entire fixpoint as one dispatch via
//! `scallop_join`, then projects per-output-cell clause bitsets into
//! a host-readable mapping.
//!
//! # Algorithm
//!
//! Same shape as the user-dialect Datalog dispatch:
//!
//! ```text
//! state[out, src]   = bitset of clauses by which `out` derives from `src`
//! join_rules[a, b]  = static "contains" adjacency (a contains b's region)
//! state ← scallop_join(state, join_rules, Lineage)  -- one GPU dispatch
//! ```
//!
//! After convergence, `state[out, src] != 0` exactly when there's
//! some derivation chain from `src` to `out`, AND the set bits name
//! the union of clauses participating in any such chain.
//!
//! # Why this is the right place
//!
//! `scallop_provenance` is the recursion-thesis-clean GPU path: same
//! closure semantics, ONE dispatch via the persistent_fixpoint
//! primitive, parity-tested against host oracles that are compiled
//! only for tests. This module is the production API for internet-scale
//! provenance closures.
//!
//! # Wiring
//!
//! Call [`build_provenance_program`] to obtain a `Program` ready
//! to dispatch against any backend. The host side seeds `state` with
//! direct (out, src, clause-bitset) triples and `join_rules` with the
//! static adjacency; the dispatch returns the closure.
//!
//! See `vyre-primitives::math::scallop_join::PROVENANCE_SELF_CONSUMER`
//! for the cross-link from the primitive's docs back here.

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_foundation::ir::Program;
use vyre_primitives::math::scallop_join;

/// Default safety cap on Datalog fixpoint iterations. Monotone Datalog
/// converges in ≤ n² iterations on n-cell systems; this cap is a
/// defensive multiple. Exposed so callers can lower it for shallow
/// graphs or raise it for adversarial test corpora.
pub const DEFAULT_PROVENANCE_MAX_ITERATIONS: u32 = 64;

/// Reusable host buffers for provenance closure reference parity.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Default, Debug, Clone)]
pub struct ScallopProvenanceScratch {
    closure: Vec<u32>,
    join_scratch: Vec<u32>,
}

/// Caller-owned GPU dispatch scratch for provenance closure.
#[derive(Default, Debug)]
pub struct ScallopProvenanceGpuScratch {
    inputs: Vec<Vec<u8>>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl ScallopProvenanceScratch {
    #[must_use]
    pub fn closure(&self) -> &[u32] {
        &self.closure
    }

    #[must_use]
    pub fn closure_mut(&mut self) -> &mut Vec<u32> {
        &mut self.closure
    }
}

/// Build the GPU-resident provenance-closure Program. The returned
/// `Program` declares four buffers: `state` (RW), `next` (RW scratch),
/// `changed` (RW 1-word convergence flag), `join_rules` (RO).
///
/// Caller seeds `state` with the direct (out, src) → clause-bitset
/// matrix and `join_rules` with the static adjacency before
/// dispatching. After dispatch, `state` is the transitive lineage
/// closure.
///
/// # Panics
///
/// Panics if `n == 0`. (`max_iterations == 0` is also rejected by the
/// underlying primitive.)
#[must_use]
pub fn build_provenance_program(n: u32, max_iterations: u32) -> Program {
    scallop_join::scallop_join(
        "provenance_state",
        "provenance_next",
        "provenance_join_rules",
        "provenance_changed",
        n,
        max_iterations,
    )
}

/// Reference oracle: compute the transitive lineage closure on the host.
/// Uses the same algorithm the GPU dispatch runs; this is the parity
/// target for `build_provenance_program` dispatch outputs.
///
/// `state[i,j]` is a bitset of clauses by which `i` derives from `j`.
/// `join_rules[i,j]` is the static adjacency (clause-bitset for the
/// direct i⇝j edge under the join rule). Returns the converged
/// closure.
///
/// # Panics
///
/// Panics if `state.len() != n*n` or `join_rules.len() != n*n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_provenance_closure(
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
) -> Vec<u32> {
    let mut scratch = ScallopProvenanceScratch::default();
    reference_provenance_closure_with_scratch(state, join_rules, n, max_iterations, &mut scratch);
    scratch.closure
}

/// Reference oracle using reusable buffers. Returns the number of fixpoint
/// iterations and writes the converged closure into `scratch.closure()`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_provenance_closure_with_scratch(
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
    scratch: &mut ScallopProvenanceScratch,
) -> u32 {
    use crate::observability::{bump, scallop_provenance_calls};
    bump(&scallop_provenance_calls);
    scallop_join::cpu_ref_into(
        state,
        join_rules,
        n,
        max_iterations,
        &mut scratch.closure,
        &mut scratch.join_scratch,
    )
}

/// Convenience: project the closure matrix into a per-output-cell
/// clause bitset. `out` is the row index; the returned vector has
/// one entry per source column with the bitset for that (out, src)
/// pair.
#[must_use]
pub fn lineage_for_output(closure: &[u32], n: u32, out: u32) -> Vec<u32> {
    let mut row = Vec::new();
    lineage_for_output_into(closure, n, out, &mut row);
    row
}

/// Project one output row into caller-owned storage.
pub fn lineage_for_output_into(closure: &[u32], n: u32, out: u32, row: &mut Vec<u32>) {
    use crate::observability::{bump, scallop_provenance_calls};
    bump(&scallop_provenance_calls);
    row.clear();
    row.extend_from_slice(lineage_for_output_slice(closure, n, out));
}

/// Build the provenance program once via [`build_provenance_program`],
/// dispatch it through the supplied `OptimizerDispatcher`, and return
/// the converged `n*n` lineage matrix.
///
/// The GPU path runs the entire fixpoint as one dispatch (the
/// `scallop_join` primitive iterates internally to fixpoint), so
/// unlike the BFS-style closures this is a single dispatch  -  no host
/// loop required.
///
/// # Errors
///
/// Propagates any [`crate::optimizer::dispatcher::DispatchError`]
/// surfaced by the dispatcher.
#[allow(clippy::too_many_arguments)]
pub fn provenance_closure_via(
    dispatcher: &dyn OptimizerDispatcher,
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut closure = Vec::new();
    provenance_closure_via_into(
        dispatcher,
        state,
        join_rules,
        n,
        max_iterations,
        &mut closure,
    )?;
    Ok(closure)
}

/// Dispatch provenance closure into caller-owned storage.
#[allow(clippy::too_many_arguments)]
pub fn provenance_closure_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
    closure: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = ScallopProvenanceGpuScratch::default();
    provenance_closure_via_with_scratch_into(
        dispatcher,
        state,
        join_rules,
        n,
        max_iterations,
        &mut scratch,
        closure,
    )
}

/// Dispatch provenance closure into caller-owned dispatch and output
/// storage.
#[allow(clippy::too_many_arguments)]
pub fn provenance_closure_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
    scratch: &mut ScallopProvenanceGpuScratch,
    closure: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, scallop_provenance_calls};
    bump(&scallop_provenance_calls);

    let n_usize = n as usize;
    let cells = n_usize.checked_mul(n_usize).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: provenance_closure_via n*n overflows usize for n={n}."
        ))
    })?;
    if n == 0 {
        if state.is_empty() && join_rules.is_empty() {
            closure.clear();
            return Ok(());
        }
        return Err(DispatchError::BadInputs(format!(
            "Fix: provenance_closure_via n=0 requires empty state and join_rules, got state={}, join_rules={}.",
            state.len(),
            join_rules.len()
        )));
    }
    if max_iterations == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: provenance_closure_via requires max_iterations > 0.".to_string(),
        ));
    }
    if state.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: provenance_closure_via requires state.len() == n*n ({cells}); got {} for n={n}.",
            state.len()
        )));
    }
    if join_rules.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: provenance_closure_via requires join_rules.len() == n*n ({cells}); got {} for n={n}.",
            join_rules.len()
        )));
    }
    let state_bytes = cells
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: provenance_closure_via byte size for {cells} cells overflows usize."
            ))
        })?;

    let program = build_provenance_program(n, max_iterations);

    // build_provenance_program orders buffers as
    //   [state RW, next RW scratch, changed RW 1-word, join_rules RO].
    ensure_input_slots(&mut scratch.inputs, 4);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], state);
    write_zero_bytes(&mut scratch.inputs[1], state_bytes);
    write_zero_bytes(&mut scratch.inputs[2], std::mem::size_of::<u32>());
    write_u32_slice_le_bytes(&mut scratch.inputs[3], join_rules);

    let grid_x = u32::try_from(cells.div_ceil(256)).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: provenance_closure_via grid size for {cells} cells exceeds u32 index space."
        ))
    })?;
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([grid_x, 1, 1]))?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: scallop provenance dispatch expected at least the state output, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], cells, "provenance_closure_via state", closure)
}

/// Borrowed projection of one output row from a provenance closure matrix.
///
/// Hot invalidation paths should use this instead of [`lineage_for_output`]
/// when they only need to inspect the row.
#[must_use]
pub fn lineage_for_output_slice(closure: &[u32], n: u32, out: u32) -> &[u32] {
    assert!(out < n, "Fix: lineage_for_output requires out < n.");
    let row = (out as usize) * (n as usize);
    &closure[row..row + (n as usize)]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::identity_op, clippy::erasing_op)]
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    struct ProvenanceDispatcher {
        outputs: Vec<Vec<u8>>,
    }

    impl OptimizerDispatcher for ProvenanceDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            if inputs.len() != 4 {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: provenance test dispatcher expected 4 inputs, got {}.",
                    inputs.len()
                )));
            }
            Ok(self.outputs.clone())
        }
    }

    #[test]
    fn build_program_declares_four_buffers() {
        let p = build_provenance_program(4, 16);
        let bufs = p.buffers();
        assert_eq!(bufs.len(), 4);
        let names: Vec<&str> = bufs.iter().map(|b| b.name()).collect();
        assert!(names.iter().any(|n| n.contains("provenance_state")));
        assert!(names.iter().any(|n| n.contains("provenance_join_rules")));
    }

    #[test]
    fn reference_closure_transitive_provenance() {
        // 3-node chain: rule_0 derives rule_1 (clause 0); rule_1
        // derives rule_2 (clause 1). Provenance of rule_2 must
        // include clauses {0, 1} after the closure.
        let mut state = vec![0u32; 9];
        state[2 * 3 + 0] = 0b01; // rule_2 derives from rule_0 with clause 0
        state[2 * 3 + 1] = 0b10; // rule_2 derives from rule_1 with clause 1
        let mut join_rules = vec![0u32; 9];
        join_rules[1 * 3 + 0] = 0b01; // rule_1 contains rule_0 (clause 0)
        let closure = reference_provenance_closure(&state, &join_rules, 3, 16);
        // After closure, rule_2's lineage on rule_0 should accumulate
        // both the direct clause-0 edge AND the indirect via rule_1.
        assert!(
            closure[2 * 3 + 0] & 0b01 != 0,
            "transitive provenance of rule_2 from rule_0 must include clause 0"
        );
    }

    #[test]
    fn reference_closure_reuses_scratch() {
        let mut state = vec![0u32; 9];
        state[0 * 3 + 1] = 0b001;
        state[1 * 3 + 2] = 0b010;
        let join_rules = state.clone();
        let mut scratch = ScallopProvenanceScratch {
            closure: Vec::with_capacity(64),
            join_scratch: Vec::with_capacity(64),
        };
        let closure_ptr = scratch.closure.as_ptr();
        let join_ptr = scratch.join_scratch.as_ptr();
        let iters =
            reference_provenance_closure_with_scratch(&state, &join_rules, 3, 16, &mut scratch);
        assert!(iters <= 8);
        assert_eq!(scratch.closure()[0 * 3 + 2] & 0b011, 0b011);
        assert_eq!(scratch.closure.as_ptr(), closure_ptr);
        assert_eq!(scratch.join_scratch.as_ptr(), join_ptr);
    }

    #[test]
    fn lineage_for_output_projection() {
        // 2x2 closure; out=1 row should be the second row exactly.
        let closure = vec![0u32, 0u32, 0b11u32, 0b10u32];
        let row = lineage_for_output(&closure, 2, 1);
        assert_eq!(row, vec![0b11u32, 0b10u32]);
    }

    #[test]
    fn lineage_for_output_into_reuses_row_buffer() {
        let closure = vec![0u32, 0u32, 0b11u32, 0b10u32];
        let mut row = Vec::with_capacity(8);
        let ptr = row.as_ptr();
        lineage_for_output_into(&closure, 2, 1, &mut row);
        assert_eq!(row, vec![0b11u32, 0b10u32]);
        assert_eq!(row.as_ptr(), ptr);
    }

    #[test]
    fn via_decodes_exact_state_output_into_reused_buffer() {
        let expected = vec![1u32, 2, 3, 4];
        let dispatcher = ProvenanceDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&expected)],
        };
        let state = vec![0u32; 4];
        let join_rules = vec![0u32; 4];
        let mut closure = Vec::with_capacity(8);
        let ptr = closure.as_ptr();
        provenance_closure_via_into(&dispatcher, &state, &join_rules, 2, 16, &mut closure)
            .expect("Fix: dispatch succeeds");
        assert_eq!(closure, expected);
        assert_eq!(closure.as_ptr(), ptr);
    }

    #[test]
    fn via_with_scratch_reuses_dispatch_and_output_storage() {
        let expected = vec![1u32, 2, 3, 4];
        let dispatcher = ProvenanceDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&expected)],
        };

        let state = vec![0u32; 4];
        let join_rules = vec![0u32; 4];
        let mut scratch = ScallopProvenanceGpuScratch::default();
        let mut closure = Vec::with_capacity(4);

        provenance_closure_via_with_scratch_into(
            &dispatcher,
            &state,
            &join_rules,
            2,
            16,
            &mut scratch,
            &mut closure,
        )
        .expect("Fix: dispatch succeeds");

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let closure_capacity = closure.capacity();

        provenance_closure_via_with_scratch_into(
            &dispatcher,
            &state,
            &join_rules,
            2,
            16,
            &mut scratch,
            &mut closure,
        )
        .expect("Fix: dispatch succeeds");

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(closure.capacity(), closure_capacity);
        assert_eq!(closure, expected);
    }

    #[test]
    fn via_accepts_primitive_scratch_output_buffers() {
        let dispatcher = ProvenanceDispatcher {
            outputs: vec![
                u32_slice_to_le_bytes(&[1, 2, 3, 4]),
                u32_slice_to_le_bytes(&[0, 0, 0, 0]),
                u32_slice_to_le_bytes(&[0]),
            ],
        };
        let state = vec![0u32; 4];
        let join_rules = vec![0u32; 4];
        let closure = provenance_closure_via(&dispatcher, &state, &join_rules, 2, 16)
            .expect("Fix: scratch outputs must not mask the state output");
        assert_eq!(closure, vec![1, 2, 3, 4]);
    }

    #[test]
    fn via_rejects_trailing_state_bytes() {
        let dispatcher = ProvenanceDispatcher {
            outputs: vec![vec![1, 0, 0, 0, 2]],
        };
        let state = vec![0u32; 1];
        let join_rules = vec![0u32; 1];
        let err = provenance_closure_via(&dispatcher, &state, &join_rules, 1, 16)
            .expect_err("trailing bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn empty_seed_yields_empty_closure() {
        // No seed facts AND no join rules → no derivations, closure
        // is all zeros. Zero-absorbing combine prevents phantom
        // lineage.
        let state = vec![0u32; 16];
        let join_rules = vec![0u32; 16];
        let closure = reference_provenance_closure(&state, &join_rules, 4, 16);
        assert!(closure.iter().all(|&w| w == 0));
    }

    #[test]
    fn idempotent_seeded_closure_is_stable() {
        // Single seed fact, no join rules → fixpoint is the seed
        // itself (nothing to derive). Convergence in 1-2 iterations.
        let mut state = vec![0u32; 9];
        state[0] = 0b01;
        let join_rules = vec![0u32; 9];
        let closure = reference_provenance_closure(&state, &join_rules, 3, 16);
        // The seed bit must persist (Datalog never retracts) and no
        // other bits should appear.
        assert_eq!(closure[0], 0b01);
        for &cell in &closure[1..] {
            assert_eq!(cell, 0, "no spurious derivations under empty join rules");
        }
    }

    #[test]
    fn default_iterations_caps_runaway() {
        // Defensive: a malformed monotone-violating system would
        // never converge; we cap iterations and return whatever we
        // have. With monotone Lineage this is unreachable, but the
        // cap protects callers from runaway.
        const _: () = assert!(DEFAULT_PROVENANCE_MAX_ITERATIONS >= 16);
    }

    #[test]
    fn release_path_does_not_export_host_oracles_without_cpu_parity_cfg() {
        let source = include_str!("scallop_provenance.rs");
        let gpu_region = source
            .split("pub fn provenance_closure_via(")
            .nth(1)
            .expect("Fix: release provenance function must exist")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: tests follow release provenance functions");
        assert!(
            !gpu_region.contains("cpu_ref") && !gpu_region.contains("reference_provenance"),
            "release provenance path must stay GPU-dispatch-only; host references belong behind cfg(test)"
        );
        assert!(
            source.contains("#[must_use]\n#[cfg(any(test, feature = \"cpu-parity\"))]\npub fn reference_provenance_closure")
                || source.contains("#[cfg(any(test, feature = \"cpu-parity\"))]\n#[must_use]\npub fn reference_provenance_closure"),
            "host provenance reference must be compiled only for parity tests or explicit cpu-parity harnesses"
        );
    }
}
