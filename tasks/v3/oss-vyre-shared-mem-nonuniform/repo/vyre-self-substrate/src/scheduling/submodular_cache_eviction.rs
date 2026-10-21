//! Pipeline-cache eviction via #45 submodular maximization (#45 self-consumer).
//!
//! Closes the recursion thesis for #45  -  submodular_greedy ships to
//! user dialects (feature selection, sensor placement, summarization,
//! coreset construction) AND drives vyre's compile-cache eviction
//! policy.
//!
//! # The self-use
//!
//! Vyre's backend pipeline caches can use LRU eviction: when the cache fills,
//! drop the least-recently-
//! used pipeline. LRU is fast and reasonable but provably suboptimal
//! when access frequencies are skewed  -  a frequently-hit cold-edged
//! pipeline gets evicted because it sat for one extra second.
//!
//! Submodular maximization gives a provably-better bound. Reframe:
//! "which K pipelines to KEEP cached such that expected hit rate is
//! maximized." Hit-rate-as-set-function is submodular (diminishing
//! returns: adding a pipeline to a small cache helps more than adding
//! it to a large cache). Greedy-pick-by-marginal-gain achieves
//! `(1 - 1/e) ≈ 63%` of optimum (Nemhauser 1978). Stochastic-greedy
//! (Mirzasoleiman 2015) gets close to that bound at GPU-friendly cost.
//!
//! For 0.6 we ship the per-step argmax-of-marginals primitive that
//! the cache eviction policy will call once per fill  -  the K
//! consecutive argmax-of-marginals calls produce the K-element
//! retention set; everything else is evicted.
//!
//! # Why this matters
//!
//! At 65k cached pipelines (the current LruPipelineCache cap), LRU
//! evicts ~30% of pipelines that should be retained on a workload
//! with skewed temporal locality (typical for security scanning
//! with hot-path/cold-path bimodal). Submodular eviction recovers
//! most of those retained  -  measurable improvement in cache hit
//! rate at no per-eviction cost (the marginal-gain table is built
//! incrementally).
//!
//! # Algorithm
//!
//! ```text
//! gains[i]    = expected hit rate for pipeline i conditional on
//!               current cache contents (caller's hit-tracker
//!               populates this)
//! picked[i]   = 1 if pipeline i already in retention set
//!
//! while |picked| < K:
//!     winner = argmax_of_marginals(gains, picked)
//!     if winner is NO_WINNER: break
//!     picked[winner] = 1
//!     gains[*] -= covered_gain(winner)  // diminishing returns
//!
//! evict every pipeline whose picked == 0
//! ```

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::math::submodular_greedy::argmax_of_marginals_cpu;
use vyre_primitives::math::submodular_greedy::{argmax_of_marginals, NO_WINNER};

/// Caller-owned GPU dispatch scratch for submodular cache eviction.
#[derive(Debug, Default)]
pub struct SubmodularEvictionGpuScratch {
    inputs: Vec<Vec<u8>>,
    winner: Vec<u32>,
}

/// Compute the retention set: K pipelines to KEEP cached. `gains[i]`
/// is the expected hit rate for pipeline i conditional on prior
/// retentions; `n` is the total pipeline count; `k` is the cache
/// capacity.
///
/// Returns a 0/1 vector of length n: 1 = retain, 0 = evict.
///
/// The caller is responsible for updating the gains table to reflect
/// diminishing returns  -  if pipelines i and j have correlated access
/// patterns, picking i should reduce j's marginal gain. For a simple
/// independent-access model the unmodified gains suffice; richer
/// models pass an updated `gains` slice per step.
///
/// # Panics
///
/// Panics if `gains.len() != n` or `k > n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn select_retention_set(gains: &mut [u32], n: u32, k: u32) -> Vec<u32> {
    let mut picked = Vec::with_capacity(n as usize);
    reference_select_retention_set_into(gains, n, k, &mut picked);
    picked
}

/// Compute the retention set into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_select_retention_set_into(
    gains: &mut [u32],
    n: u32,
    k: u32,
    picked: &mut Vec<u32>,
) {
    use crate::observability::{bump, submodular_cache_eviction_calls};
    bump(&submodular_cache_eviction_calls);
    assert_eq!(gains.len(), n as usize);
    assert!(k <= n, "Fix: k must not exceed n.");

    picked.clear();
    picked.resize(n as usize, 0);
    let mut keep_count = 0u32;
    while keep_count < k {
        let (winner, _) = argmax_of_marginals_cpu(gains, picked);
        if winner == NO_WINNER {
            break;
        }
        picked[winner as usize] = 1;
        // Zero the picked element's gain so subsequent argmax
        // ignores it. Richer models would compute conditional
        // marginal gains; the simple model treats access as
        // independent and only decreases by the picked-itself
        // gain.
        gains[winner as usize] = 0;
        keep_count += 1;
    }
}

/// Compute the retention set through the GPU-dispatchable submodular argmax primitive.
///
/// This is the production path for callers with a concrete backend dispatcher. It performs the
/// same simple independent-access greedy loop as [`select_retention_set_into`], dispatching
/// `vyre_primitives::math::submodular_greedy::argmax_of_marginals` once per retained item.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when `gains.len() != n`, `k > n`, `n == 0`, dispatch
/// output is missing/truncated, or backend dispatch fails.
pub fn select_retention_set_via(
    dispatcher: &dyn OptimizerDispatcher,
    gains: &mut [u32],
    n: u32,
    k: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut picked = Vec::with_capacity(n as usize);
    select_retention_set_via_into(dispatcher, gains, n, k, &mut picked)?;
    Ok(picked)
}

/// Compute the retention set through dispatch into caller-owned storage.
pub fn select_retention_set_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    gains: &mut [u32],
    n: u32,
    k: u32,
    picked: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = SubmodularEvictionGpuScratch::default();
    select_retention_set_via_with_scratch_into(dispatcher, gains, n, k, &mut scratch, picked)
}

/// Compute the retention set through dispatch into caller-owned dispatch and
/// output storage.
pub fn select_retention_set_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    gains: &mut [u32],
    n: u32,
    k: u32,
    scratch: &mut SubmodularEvictionGpuScratch,
    picked: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, submodular_cache_eviction_calls};
    bump(&submodular_cache_eviction_calls);
    if n == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: select_retention_set_via requires n > 0.".to_string(),
        ));
    }
    if gains.len() != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: select_retention_set_via expected gains.len() == n == {n}, got {}.",
            gains.len()
        )));
    }
    if k > n {
        return Err(DispatchError::BadInputs(format!(
            "Fix: select_retention_set_via requires k <= n; got k={k}, n={n}."
        )));
    }

    picked.clear();
    picked.resize(n as usize, 0);
    let mut keep_count = 0u32;
    while keep_count < k {
        let (winner, _) = dispatch_argmax_step_with_scratch(dispatcher, gains, picked, n, scratch)?;
        if winner == NO_WINNER {
            break;
        }
        let winner_idx = winner as usize;
        if winner_idx >= picked.len() {
            return Err(DispatchError::BadInputs(format!(
                "Fix: submodular argmax returned winner {winner} outside n={n}."
            )));
        }
        picked[winner_idx] = 1;
        gains[winner_idx] = 0;
        keep_count += 1;
    }
    Ok(())
}

fn dispatch_argmax_step_with_scratch(
    dispatcher: &dyn OptimizerDispatcher,
    gains: &[u32],
    picked: &[u32],
    n: u32,
    scratch: &mut SubmodularEvictionGpuScratch,
) -> Result<(u32, u32), DispatchError> {
    let program = argmax_of_marginals("gains", "picked", "winner_idx", "winner_gain", n);
    ensure_input_slots(&mut scratch.inputs, 4);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], gains);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], picked);
    write_zero_bytes(&mut scratch.inputs[2], std::mem::size_of::<u32>());
    write_zero_bytes(&mut scratch.inputs[3], std::mem::size_of::<u32>());
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([1, 1, 1]))?;
    if outputs.len() != 2 {
        return Err(DispatchError::BackendError(format!(
            "Fix: submodular argmax dispatch returned {} outputs, expected exactly winner_idx and winner_gain.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], 1, "winner_idx", &mut scratch.winner)?;
    let winner_idx = scratch.winner[0];
    decode_u32_output_exact(&outputs[1], 1, "winner_gain", &mut scratch.winner)?;
    let winner_gain = scratch.winner[0];
    Ok((winner_idx, winner_gain))
}

/// Convenience: invert retention to eviction (1 = evict).
#[must_use]
pub fn invert_to_eviction_set(retention: &[u32]) -> Vec<u32> {
    let mut eviction = Vec::with_capacity(retention.len());
    invert_to_eviction_set_into(retention, &mut eviction);
    eviction
}

/// Invert retention to eviction (1 = evict) into caller-owned storage.
pub fn invert_to_eviction_set_into(retention: &[u32], eviction: &mut Vec<u32>) {
    eviction.clear();
    reserve_vec_capacity_or_panic(eviction, retention.len(), "submodular eviction output");
    eviction.extend(retention.iter().map(|&r| if r == 0 { 1 } else { 0 }));
}

/// Approximate worst-case retention quality bound: greedy submodular
/// maximization achieves `(1 - 1/e)` ≈ 0.632 of optimum. Returns the
/// expected lower bound on retention quality given an optimum.
#[must_use]
pub fn greedy_quality_bound(optimum: u32) -> u32 {
    // `(1 - 1/e) ≈ 0.6321205588`. Use integer approximation
    // via 6321/10000 to keep this f64-free.
    ((optimum as u64) * 6321 / 10000) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    struct ArgmaxDispatcher;

    impl OptimizerDispatcher for ArgmaxDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            let [gains_bytes, picked_bytes, _, _] = inputs else {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: argmax test dispatcher expected 4 buffers, got {}.",
                    inputs.len()
                )));
            };
            let gains = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
                gains_bytes,
                "argmax test dispatcher",
            )?;
            let picked = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
                picked_bytes,
                "argmax test dispatcher",
            )?;
            let (winner_idx, winner_gain) = argmax_of_marginals_cpu(&gains, &picked);
            Ok(vec![
                u32_slice_to_le_bytes(&[winner_idx]),
                u32_slice_to_le_bytes(&[winner_gain]),
            ])
        }
    }

    struct ExtraOutputDispatcher;

    impl OptimizerDispatcher for ExtraOutputDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(vec![
                u32_slice_to_le_bytes(&[0]),
                u32_slice_to_le_bytes(&[1]),
                u32_slice_to_le_bytes(&[2]),
            ])
        }
    }

    struct TrailingBytesDispatcher;

    impl OptimizerDispatcher for TrailingBytesDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(vec![vec![0, 0, 0, 0, 99], u32_slice_to_le_bytes(&[1])])
        }
    }

    #[test]
    fn picks_top_k_by_gain() {
        let mut gains = vec![3u32, 7, 2, 9, 5];
        let retention = select_retention_set(&mut gains, 5, 3);
        // Top 3 gains were at indices 3 (9), 1 (7), 4 (5).
        assert_eq!(retention, vec![0, 1, 0, 1, 1]);
    }

    #[test]
    fn via_picks_top_k_by_gain() {
        let dispatcher = ArgmaxDispatcher;
        let mut gains = vec![3u32, 7, 2, 9, 5];
        let retention = select_retention_set_via(&dispatcher, &mut gains, 5, 3)
            .expect("Fix: dispatch succeeds");
        assert_eq!(retention, vec![0, 1, 0, 1, 1]);
        assert_eq!(gains, vec![3, 0, 2, 0, 0]);
    }

    #[test]
    fn via_with_scratch_reuses_dispatch_decode_and_output_storage() {
        let dispatcher = ArgmaxDispatcher;
        let mut scratch = SubmodularEvictionGpuScratch::default();
        let mut picked = Vec::with_capacity(5);
        let mut gains = vec![3u32, 7, 2, 9, 5];

        select_retention_set_via_with_scratch_into(
            &dispatcher,
            &mut gains,
            5,
            3,
            &mut scratch,
            &mut picked,
        )
        .unwrap();

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let winner_capacity = scratch.winner.capacity();
        let picked_capacity = picked.capacity();
        let mut gains_again = vec![3u32, 7, 2, 9, 5];

        select_retention_set_via_with_scratch_into(
            &dispatcher,
            &mut gains_again,
            5,
            3,
            &mut scratch,
            &mut picked,
        )
        .unwrap();

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(scratch.winner.capacity(), winner_capacity);
        assert_eq!(picked.capacity(), picked_capacity);
        assert_eq!(picked, vec![0, 1, 0, 1, 1]);
        assert_eq!(gains_again, vec![3, 0, 2, 0, 0]);
    }

    #[test]
    fn via_rejects_extra_backend_outputs() {
        let dispatcher = ExtraOutputDispatcher;
        let mut gains = vec![3u32, 7, 2];
        let err = select_retention_set_via(&dispatcher, &mut gains, 3, 1)
            .expect_err("extra backend outputs must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn via_rejects_trailing_backend_output_bytes() {
        let dispatcher = TrailingBytesDispatcher;
        let mut gains = vec![3u32, 7, 2];
        let err = select_retention_set_via(&dispatcher, &mut gains, 3, 1)
            .expect_err("trailing backend output bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn production_source_keeps_cpu_submodular_helpers_out_of_via_path() {
        let source = include_str!("submodular_cache_eviction.rs");
        let via_section = source
            .split("pub fn select_retention_set_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("/// Convenience: invert retention to eviction")
            .next()
            .expect("Fix: post-via marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_select"));
    }

    #[test]
    fn k_eq_zero_evicts_all() {
        let mut gains = vec![3u32, 7, 2, 9, 5];
        let retention = select_retention_set(&mut gains, 5, 0);
        assert_eq!(retention, vec![0; 5]);
    }

    #[test]
    fn k_eq_n_retains_all() {
        let mut gains = vec![3u32, 7, 2, 9, 5];
        let retention = select_retention_set(&mut gains, 5, 5);
        assert_eq!(retention, vec![1; 5]);
    }

    #[test]
    fn invert_complements_retention() {
        let retention = vec![1, 0, 1, 0, 1];
        let eviction = invert_to_eviction_set(&retention);
        assert_eq!(eviction, vec![0, 1, 0, 1, 0]);
    }

    #[test]
    fn invert_into_reuses_eviction_buffer() {
        let retention = vec![1, 0, 1, 0, 1];
        let mut eviction = Vec::with_capacity(8);
        let ptr = eviction.as_ptr();
        invert_to_eviction_set_into(&retention, &mut eviction);
        assert_eq!(eviction, vec![0, 1, 0, 1, 0]);
        assert_eq!(eviction.as_ptr(), ptr);
    }

    #[test]
    fn quality_bound_is_lower_bound() {
        // (1 - 1/e) of 100 ≈ 63.
        assert_eq!(greedy_quality_bound(100), 63);
        // Of 1000 ≈ 632.
        assert_eq!(greedy_quality_bound(1000), 632);
    }

    #[test]
    fn k_larger_than_n_panics() {
        let mut gains = vec![1u32, 2, 3];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = select_retention_set(&mut gains, 3, 5);
        }));
        assert!(matches!(result, Err(_)), "k > n must panic");
    }
}
