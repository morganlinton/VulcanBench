//! Substrate-call observability counters.
//!
//! Each self-consumer module increments a global atomic counter on
//! every call. Operators read snapshots via [`snapshot_counters`] for
//! Prometheus / OpenTelemetry / Datadog dashboards. Lets us answer:
//!
//! - Which substrate modules are actually consumed in production
//!   (≠ shipped library code)?
//! - Which substrate calls dominate the dispatch hot path?
//! - When a substrate path is added, when does it first see traffic?
//!
//! The counters are lock-free (`AtomicU64` with relaxed ordering) so
//! they don't add overhead to the hot path. Reading the snapshot is
//! also lock-free.

use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! counters {
    ($($name:ident),* $(,)?) => {
        $(
            #[allow(non_upper_case_globals)]
            pub(crate) static $name: AtomicU64 = AtomicU64::new(0);
        )*

        /// Snapshot of every substrate counter as a flat slice of
        /// (module_name, call_count) tuples. Each call resets nothing
        ///  -  counters are monotonic from process start. Callers can
        /// diff two snapshots to derive call rate.
        #[must_use]
        pub fn snapshot_counters() -> Vec<(&'static str, u64)> {
            vec![
                $(
                    (stringify!($name), $name.load(Ordering::Relaxed)),
                )*
            ]
        }

        /// Sum of every substrate-call counter. Useful as a
        /// single-number "is the substrate doing anything" health
        /// signal in dashboards.
        #[must_use]
        pub fn total_calls() -> u64 {
            let mut sum = 0u64;
            $(
                sum = sum.saturating_add($name.load(Ordering::Relaxed));
            )*
            sum
        }
    };
}

counters! {
    matroid_megakernel_scheduler_calls,
    megakernel_schedule_calls,
    multigrid_matroid_solver_calls,
    sheaf_heterophilic_dispatch_calls,
    sheaf_spectral_clustering_calls,
    submodular_cache_eviction_calls,
    do_calculus_change_impact_calls,
    scallop_provenance_calls,
    vsa_fingerprint_calls,
    bitset_mask_algebra_calls,
    reduction_metrics_calls,
    matching_diagnostic_compaction_calls,
    matroid_exact_megakernel_calls,
    amg_pass_solver_calls,
    tensor_train_compression_calls,
    conv1d_latency_smoothing_calls,
    natural_gradient_autotuner_calls,
    kfac_autotune_step_calls,
    qsvt_matrix_function_fusion_calls,
    differentiable_autotune_calls,
    bellman_tn_order_calls,
    cost_model_calls,
    fmm_polyhedral_compress_calls,
    mori_zwanzig_region_coarsen_calls,
    persistent_homology_loop_signature_calls,
    planar_rewrite_pass_scheduler_calls,
    polyhedral_fusion_calls,
    sinkhorn_dispatch_clustering_calls,
    sinkhorn_full_clustering_calls,
    spectral_schedule_calls,
    string_diagram_ir_rewrite_calls,
    tensor_network_fusion_order_calls,
    tensor_train_chain_fusion_calls,
    knowledge_compile_pass_precondition_calls,
    adjustment_set_pass_dependency_calls,
    dataflow_fixpoint_calls,
    alias_registry_calls,
    toposort_calls,
    graph_dispatch_calls,
    functorial_pass_composition_calls,
    scallop_provenance_wide_calls,
    level_wave_pass_calls,
    vast_tree_walk_calls,
}

/// Bump counter `c` by one. Used internally by self-consumer modules
/// in their public-API entry points.
#[inline]
pub(crate) fn bump(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_nonempty_with_known_names() {
        let snap = snapshot_counters();
        assert!(!snap.is_empty());
        let names: Vec<&str> = snap.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"matroid_megakernel_scheduler_calls"));
        assert!(names.contains(&"vsa_fingerprint_calls"));
        assert!(names.contains(&"scallop_provenance_calls"));
        assert!(names.contains(&"alias_registry_calls"));
        assert!(names.contains(&"toposort_calls"));
        assert!(names.contains(&"graph_dispatch_calls"));
    }

    #[test]
    fn bump_increments_counter() {
        let before = matroid_megakernel_scheduler_calls.load(Ordering::Relaxed);
        bump(&matroid_megakernel_scheduler_calls);
        let after = matroid_megakernel_scheduler_calls.load(Ordering::Relaxed);
        assert_eq!(after, before + 1);
    }

    #[test]
    fn total_calls_is_monotonic() {
        let before = total_calls();
        bump(&vsa_fingerprint_calls);
        let after = total_calls();
        assert!(after > before);
    }
}
