//! Program builders surfaced through the megakernel planner.

/// for callers that need the full iterative-balance variant rather
/// than the dispatch-clustering simplification.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_sinkhorn_full_clustering_program(
    k: &str,
    k_t: &str,
    a: &str,
    b: &str,
    u_curr: &str,
    u_next: &str,
    v: &str,
    kv: &str,
    ktu: &str,
    changed: &str,
    m: u32,
    n: u32,
    max_iterations: u32,
) -> vyre_foundation::ir::Program {
    vyre_self_substrate::sinkhorn_full_clustering::sinkhorn_full_clustering_program(
        k,
        k_t,
        a,
        b,
        u_curr,
        u_next,
        v,
        kv,
        ktu,
        changed,
        m,
        n,
        max_iterations,
    )
}

/// Build a multi-word scallop-provenance Program. Wraps
/// [`vyre_self_substrate::scallop_provenance_wide::scallop_provenance_wide_program`]
/// for >32-rule lineage tracking (W=8 → 256 rules max).
#[must_use]
pub fn build_scallop_provenance_wide_program(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    w: u32,
    max_iterations: u32,
) -> vyre_foundation::ir::Program {
    vyre_self_substrate::scallop_provenance_wide::scallop_provenance_wide_program(
        state,
        next,
        join_rules,
        changed,
        n,
        w,
        max_iterations,
    )
}
/// Bellman tensor-network ordering Program builder. Wraps
/// [`vyre_self_substrate::bellman_tn_order::bellman_tn_order_program`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_bellman_tn_order_program(
    src: &str,
    dst: &str,
    weight: &str,
    dist: &str,
    next_dist: &str,
    changed: &str,
    n_nodes: u32,
    n_edges: u32,
    max_iterations: u32,
) -> vyre_foundation::ir::Program {
    vyre_self_substrate::bellman_tn_order::bellman_tn_order_program(
        src,
        dst,
        weight,
        dist,
        next_dist,
        changed,
        n_nodes,
        n_edges,
        max_iterations,
    )
}

/// KFAC autotune-step Program builder. Wraps
/// [`vyre_self_substrate::kfac_autotune_step::kfac_autotune_step_program`].
#[must_use]
pub fn build_kfac_autotune_step_program(
    blocks_out: &str,
    blocks_in: &str,
    scratch: &str,
    num_blocks: u32,
    n: u32,
) -> vyre_foundation::ir::Program {
    vyre_self_substrate::kfac_autotune_step::kfac_autotune_step_program(
        blocks_out, blocks_in, scratch, num_blocks, n,
    )
}

/// Build a sinkhorn dispatch-clustering Program. Wraps
/// [`vyre_self_substrate::sinkhorn_dispatch_clustering::sinkhorn_clustering_program`].
#[must_use]
pub fn build_sinkhorn_clustering_program(
    m: u32,
    n: u32,
    d: u32,
    iters: u32,
    eps: f32,
) -> vyre_foundation::ir::Program {
    vyre_self_substrate::sinkhorn_dispatch_clustering::sinkhorn_clustering_program(
        m, n, d, iters, eps,
    )
}
/// Build a persistent-fixpoint Program around a caller-supplied
/// transfer body. Replaces a host-side `loop { dispatch(); check }`
/// pattern with a single GPU-side dispatch that ping-pongs
/// `current ↔ next` until `changed[0] == 0` or `max_iterations`.
///
/// P-DRIVER-9: every host fixpoint loop should migrate to this
/// substrate Program. Caller supplies `transfer_body` that reads
/// `current` and writes `next`; the wrapper handles the convergence
/// flag and ping-pong copy.
#[must_use]
pub fn build_persistent_fixpoint_program(
    transfer_body: Vec<vyre_foundation::ir::Node>,
    current: &str,
    next: &str,
    changed: &str,
    words: u32,
    max_iterations: u32,
) -> vyre_foundation::ir::Program {
    vyre_self_substrate::persistent_fixpoint_program::persistent_fixpoint_program(
        transfer_body,
        current,
        next,
        changed,
        words,
        max_iterations,
    )
}
