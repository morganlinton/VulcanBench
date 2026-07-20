//! Self-hosted optimizer pipeline running on persistent GPU buffers.
//!
//! For dispatchers that support the persistent surface (CUDA today),
//! this orchestrator encodes the input Program once, allocates and
//! uploads the arena RO buffers once, dispatches all four passes
//! (canonicalize, const-fold, pattern-match, DCE) against the same
//! resident buffers, and reads back the final state buffers in one
//! shot. Eliminates the per-pass dispatch sync overhead that
//! dominates the borrowed-`dispatch` path.
//!
//! Semantics: canon, const-fold, and pattern-match are run in parallel
//! against the SAME pre-canon arena. Their deltas are merged in a
//! priority order (const-fold > pattern-match > canonicalize) when
//! conflicts arise. This is correct for the V1 rule sets:
//!  - V1 const-fold rules require both operands literal  -  neither
//!    canon's swap nor pattern-match's identity rules change literal-
//!    operand status.
//!  - V1 canon swap is "literal on right"  -  already-canon Programs
//!    are unchanged.
//!  - V1 pattern-match rules check both literal-on-left and literal-
//!    on-right, so they fire regardless of canon's swap.
//! After the merge produces an intermediate Program, DCE runs on it
//! through its own ProgramGraph encoding (still resident-dispatched).
//!
//! For dispatchers without persistent support, callers should fall
//! back to the existing `gpu_canonicalize` → `gpu_const_fold` →
//! `gpu_algebraic_identities` → `gpu_dce` chain.

use vyre_foundation::ir::Program;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::program_graph::ProgramGraphShape;

use crate::dispatch_buffers::{decode_u32_output_exact, u32_slice_to_le_bytes};

use super::canonicalize_via_encoded::build_canonicalize_program;
use super::const_fold_via_encoded::build_const_fold_program_fused;
use super::cse_via_encoded::{
    apply_cse_let_dedupe_with_lookup, build_canonical_id_program, build_structural_hash_program,
    CANONICAL_TABLE_MULT,
};
use super::dce_program::build_dce_bfs_program;
use super::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
    ResidentStaticBufferSet,
};
use super::encode::{apply_live_bitset_mask, encode_program, ROOT_GRAPH_ID};
use super::expr_arena::encode_expr_arena;
use super::pattern_match_via_encoded::build_pattern_match_program_with_cse;
use super::pipeline_resident_decode::{
    apply_combined_arena_deltas_bitsets, build_resident_delta_bitset_pack_program,
};

const RESIDENT_CACHE_DOMAIN_PIPELINE_ARENA_RO: u64 = 0x5659_5245_4152_4f31;
const RESIDENT_CACHE_DOMAIN_PIPELINE_DCE_RO: u64 = 0x5659_5245_4443_4531;

/// Errors surfaced by the persistent pipeline.
#[derive(Debug)]
pub enum PipelineError {
    /// Encoder did not accept the input shape.
    Encode(super::encode::EncodeError),
    /// Dispatcher rejected or failed.
    Dispatch(DispatchError),
    /// GPU validator caught a foundation-level limit violation.
    /// Field is `[v033_violation, v019_violation]`.
    LimitViolation([bool; 2]),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "pipeline_resident encode error: {err:?}"),
            Self::Dispatch(err) => write!(f, "pipeline_resident dispatch error: {err}"),
            Self::LimitViolation([v033, v019]) => write!(
                f,
                "pipeline_resident GPU validator: V033 (expr depth) = {v033}, \
                 V019 (node count) = {v019}. Fix: split the program into smaller \
                 kernels or flatten deep expressions before lowering.",
            ),
        }
    }
}

impl std::error::Error for PipelineError {}

/// Run the four-pass self-hosted optimizer on `program` using the
/// dispatcher's persistent path. Caller is expected to have probed
/// `dispatcher.supports_persistent() == true` first; otherwise this
/// returns `DispatchError::Rejected` from the first persistent call.
pub fn gpu_pipeline_resident(
    program: Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<Program, PipelineError> {
    let arena = encode_expr_arena(&program).map_err(PipelineError::Encode)?;
    let preflight_node_count = u32::try_from(arena.node_top_level_exprs.len()).map_err(|_| {
        PipelineError::Dispatch(DispatchError::Rejected(
            "Fix: pipeline_resident arena node count exceeds u32::MAX; \
                 split the program before GPU validation."
                .to_string(),
        ))
    })?;

    // Preflight: GPU limit validator. Fails fast on foundation-level
    // V033 / V019 limit violations instead of letting them surface as
    // a downstream backend error. Reuse the arena that the optimizer
    // kernels consume instead of running a second ProgramGraph encode
    // only to recover the same node count.
    let limits = super::validate_via_encoded::gpu_validate_limits_from_encoding(
        &arena,
        preflight_node_count,
        dispatcher,
    )
    .map_err(|err| match err {
        super::validate_via_encoded::ValidateError::Encode(e) => PipelineError::Encode(e),
        super::validate_via_encoded::ValidateError::Dispatch(e) => PipelineError::Dispatch(e),
    })?;
    if limits[0] || limits[1] {
        return Err(PipelineError::LimitViolation(limits));
    }

    let n = arena.expr_count;
    if n == 0 {
        // No Exprs: only DCE could possibly do something. Run the DCE
        // half via the persistent path, skip the arena passes.
        return run_dce_resident(program, dispatcher).map_err(PipelineError::Dispatch);
    }

    // ---- Allocate resident handles for the arena passes ------------
    // Shared immutable RO across canon + const-fold + pattern-match.
    let kinds_bytes = u32_slice_to_le_bytes(&arena.kinds);
    let arg0_bytes = u32_slice_to_le_bytes(&arena.arg0);
    let arg1_bytes = u32_slice_to_le_bytes(&arena.arg1);
    let arg2_bytes = u32_slice_to_le_bytes(&arena.arg2);
    let depths_bytes = u32_slice_to_le_bytes(&arena.depths);
    let max_depth_bytes = u32_slice_to_le_bytes(&[arena.max_depth]);
    // Single-u32 buffer holding `max_depth` for the fused const-fold
    // kernel. Read once per level inside the kernel; the kernel breaks
    // out when `level > max_depth`.
    // RW outputs.
    let zero_n = vec![0u8; n as usize * 4];
    // CSE buffers: structural hash + canonical id + table scratch.
    // Pre-allocated here so the pattern-match dispatch can consume the
    // canonical buffer without re-allocating.
    let table_capacity = n.saturating_mul(CANONICAL_TABLE_MULT).max(2);
    let table_init_byte_len = table_capacity as usize * 4;
    let bitset_words_len = bitset_words(n) as usize;
    let bitset_byte_len = bitset_words_len
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            PipelineError::Dispatch(DispatchError::BackendError(format!(
                "Fix: pipeline_resident compact arena bitset byte count overflows usize for expr_count={n}."
            )))
        })?;
    let arena_static_payloads: [&[u8]; 6] = [
        &kinds_bytes,
        &arg0_bytes,
        &arg1_bytes,
        &arg2_bytes,
        &depths_bytes,
        &max_depth_bytes,
    ];
    let arena_static = acquire_static_uploads(
        dispatcher,
        RESIDENT_CACHE_DOMAIN_PIPELINE_ARENA_RO,
        &arena_static_payloads,
        "pipeline_resident arena read-only cache",
    )?;
    let arena_mutable_lens: [usize; 9] = [
        zero_n.len(),
        zero_n.len(),
        zero_n.len(),
        zero_n.len(),
        zero_n.len(),
        zero_n.len(),
        table_init_byte_len,
        bitset_byte_len,
        bitset_byte_len,
    ];
    let arena_mutable_handles = alloc_many_lens(dispatcher, &arena_mutable_lens)?;
    let kinds_h = arena_static.handles[0];
    let arg0_h = arena_static.handles[1];
    let arg1_h = arena_static.handles[2];
    let arg2_h = arena_static.handles[3];
    let depths_h = arena_static.handles[4];
    let max_depth_h = arena_static.handles[5];
    let swap_mask_h = arena_mutable_handles[0];
    let foldable_h = arena_mutable_handles[1];
    let value_h = arena_mutable_handles[2];
    let rewrite_action_h = arena_mutable_handles[3];
    let hash_h = arena_mutable_handles[4];
    let canonical_h = arena_mutable_handles[5];
    let table_canonical_h = arena_mutable_handles[6];
    let swap_bits_h = arena_mutable_handles[7];
    let fold_bits_h = arena_mutable_handles[8];

    let arena_pass_workgroup_x: u32 = 256;
    let grid_x = (n + arena_pass_workgroup_x - 1) / arena_pass_workgroup_x;
    let grid = Some([grid_x, 1, 1]);
    let trace = std::env::var("VYRE_STAGE_TRACE").is_ok();
    let t_total = std::time::Instant::now();
    let mut t = std::time::Instant::now();

    // ---- Arena resident kernel sequence --------------------------
    let canon_program = build_canonicalize_program(n);
    // The fused kernel runs the level loop internally with workgroup-
    // scope barriers between levels. Single-workgroup design (grid =
    // [1,1,1]) means no GridSync needed. Eliminates the per-level
    // host dispatch overhead that dominated chain-shaped Programs.
    //
    // `max_depth_iter_cap` is the static upper bound on the outer
    // Loop in the IR. We pass `max(arena.max_depth + 1, 1)` so the
    // kernel covers exactly the needed range.
    let const_fold_program =
        build_const_fold_program_fused(n, arena.max_depth.saturating_add(1).max(1));
    // Single fused dispatch (single workgroup). Same pattern as the
    // fused const-fold: outer Loop over levels, workgroup-scope
    // SeqCst barriers between levels.
    let hash_program = build_structural_hash_program(n, arena.max_depth.saturating_add(1).max(1));
    // Brute-force scan O(n²); each thread i finds smallest j ≤ i with
    // matching hash. Acceptable up to n ≈ 5000 on RTX 5090.
    let canonical_program = build_canonical_id_program(n, table_capacity);
    let pattern_program = build_pattern_match_program_with_cse(n);
    let delta_bitset_program = build_resident_delta_bitset_pack_program(n);
    let canon_handles = [kinds_h, arg0_h, arg1_h, arg2_h, swap_mask_h];
    let const_fold_handles = [
        kinds_h,
        arg0_h,
        arg1_h,
        arg2_h,
        depths_h,
        max_depth_h,
        foldable_h,
        value_h,
    ];
    let hash_handles = [
        kinds_h,
        arg0_h,
        arg1_h,
        arg2_h,
        depths_h,
        max_depth_h,
        hash_h,
    ];
    // `build_canonical_id_program` declares 7 buffers in binding-index
    // order: hash(0), canonical(1), table_canonical(2), then the four
    // structural-tuple RO buffers arena_kinds(3), arena_arg0(4),
    // arena_arg1(5), arena_arg2(6). The tuple buffers back the Gate-2
    // full structural comparison that prevents a 32-bit hash collision
    // from declaring two distinct exprs equivalent (a CSE correctness
    // guard). All four are the same resident uploads canon/const-fold/
    // hash/pattern already bind, so reuse those handles here. Order MUST
    // match the BufferDecl binding indices: the resident dispatch
    // consumes handle_ids in non-shared binding order.
    let canonical_handles = [
        hash_h,
        canonical_h,
        table_canonical_h,
        kinds_h,
        arg0_h,
        arg1_h,
        arg2_h,
    ];
    let pattern_handles = [
        kinds_h,
        arg0_h,
        arg1_h,
        arg2_h,
        rewrite_action_h,
        canonical_h,
    ];
    let delta_bitset_handles = [swap_mask_h, foldable_h, swap_bits_h, fold_bits_h];
    let mut swap_bits = Vec::with_capacity(bitset_words_len);
    let mut fold_bits = Vec::with_capacity(bitset_words_len);
    let mut value = Vec::with_capacity(n as usize);
    let mut rewrite_action = Vec::with_capacity(n as usize);
    let mut canonical = Vec::with_capacity(n as usize);
    let mut byte_readbacks = Vec::with_capacity(5);
    let arena_steps = [
        ResidentDispatchStep {
            program: &canon_program,
            handle_ids: &canon_handles,
            grid_override: grid,
        },
        ResidentDispatchStep {
            program: &const_fold_program,
            handle_ids: &const_fold_handles,
            grid_override: Some([1, 1, 1]),
        },
        ResidentDispatchStep {
            program: &hash_program,
            handle_ids: &hash_handles,
            grid_override: Some([1, 1, 1]),
        },
        ResidentDispatchStep {
            program: &canonical_program,
            handle_ids: &canonical_handles,
            grid_override: grid,
        },
        ResidentDispatchStep {
            program: &pattern_program,
            handle_ids: &pattern_handles,
            grid_override: grid,
        },
        ResidentDispatchStep {
            program: &delta_bitset_program,
            handle_ids: &delta_bitset_handles,
            grid_override: grid,
        },
    ];
    let arena_fills = [
        (swap_mask_h, zero_n.len(), 0),
        (foldable_h, zero_n.len(), 0),
        (value_h, zero_n.len(), 0),
        (rewrite_action_h, zero_n.len(), 0),
        (hash_h, zero_n.len(), 0),
        (canonical_h, zero_n.len(), 0),
        (table_canonical_h, table_init_byte_len, 0xFF),
        (swap_bits_h, bitset_byte_len, 0),
        (fold_bits_h, bitset_byte_len, 0),
    ];
    let arena_result = upload_resident_sequence_read_u32_ranges_exact(
        dispatcher,
        &arena_fills,
        &[],
        &arena_steps,
        &[
            (
                swap_bits_h,
                0,
                bitset_words_len,
                "pipeline_resident swap_bits",
            ),
            (
                fold_bits_h,
                0,
                bitset_words_len,
                "pipeline_resident fold_bits",
            ),
            (value_h, 0, n as usize, "pipeline_resident value"),
            (
                rewrite_action_h,
                0,
                n as usize,
                "pipeline_resident rewrite_action",
            ),
            (canonical_h, 0, n as usize, "pipeline_resident canonical"),
        ],
        &mut [
            &mut swap_bits,
            &mut fold_bits,
            &mut value,
            &mut rewrite_action,
            &mut canonical,
        ],
        &mut byte_readbacks,
    );
    if trace {
        eprintln!("[pl] arena_sequence_read: {} us", t.elapsed().as_micros());
    }
    for h in [
        swap_mask_h,
        foldable_h,
        value_h,
        rewrite_action_h,
        hash_h,
        canonical_h,
        table_canonical_h,
        swap_bits_h,
        fold_bits_h,
    ] {
        let _ = dispatcher.free_resident(h);
    }
    let arena_release = dispatcher.release_resident_static_uploads(arena_static);
    arena_result.map_err(PipelineError::Dispatch)?;
    arena_release.map_err(PipelineError::Dispatch)?;

    // ---- Apply combined deltas to produce the post-arena Program ---
    t = std::time::Instant::now();
    let post_arena = apply_combined_arena_deltas_bitsets(
        &program,
        &swap_bits,
        &fold_bits,
        &value,
        &rewrite_action,
    );
    if trace {
        eprintln!("[pl] apply_deltas: {} us", t.elapsed().as_micros());
    }

    // ---- CSE let-level dedupe -------------------------------------
    // Walks Lets in the same order the encoder used; for each Let
    // whose value-Expr is structurally equivalent to an earlier Let
    // in the same scope (per `canonical`), replaces the value with
    // `Var(orig_let_name)`. Pure CPU walk on the post-arena Program
    //  -  the canonical map describes the original arena's Expr
    // equivalence classes, but Let-RHS structure (and the
    // node_top_level_exprs index) is preserved by all the
    // arena-level rewrites that come before this point, so the
    // canonical lookup remains valid.
    t = std::time::Instant::now();
    let post_dedupe = apply_cse_let_dedupe_with_lookup(&post_arena, &arena, canonical.as_slice());
    if trace {
        eprintln!("[pl] cse_let_dedupe: {} us", t.elapsed().as_micros());
    }

    // ---- Cross-scope expression CSE -------------------------------
    // Hoists same-scope duplicate top-level Exprs (Store values,
    // If conds, Loop bounds, etc.) to a shared `let cse_N = E;`
    // at the scope start. Generalizes past `apply_cse_let_dedupe`
    // which only handled `let`-RHS pairs.
    t = std::time::Instant::now();
    let post_cross = super::cross_scope_cse::apply_cross_scope_cse_with_lookup(
        &post_dedupe,
        &arena,
        canonical.as_slice(),
    );
    if trace {
        eprintln!("[pl] cross_scope_cse: {} us", t.elapsed().as_micros());
    }

    // ---- Loop-invariant code motion (LICM) -----------------------
    // For each `Loop`, hoist Let bindings whose value Expr doesn't
    // reference the iter var (and is pure) to a sibling above the
    // Loop. Conservative: stops hoisting on first side-effecting
    // Node so observable behaviour is preserved.
    t = std::time::Instant::now();
    let post_licm = super::licm::apply_licm(&post_cross);
    if trace {
        eprintln!("[pl] licm: {} us", t.elapsed().as_micros());
    }

    // ---- Constant propagation -------------------------------------
    // After const-fold may have turned `let v = (1+2)` into
    // `let v = LitU32(3)`, propagate the literal to every `Var(v)`
    // use. Catches the cascading folds that arise when fold +
    // let-dedupe run in sequence: e.g. `let a = 5; let b = a; store
    // x b` becomes `let a = 5; let b = 5; store x 5` after this
    // pass. Subsequent DCE drops `b` (and `a` if no other use).
    t = std::time::Instant::now();
    let post_prop = super::const_prop::apply_const_prop(&post_licm);
    if trace {
        eprintln!("[pl] const_prop: {} us", t.elapsed().as_micros());
    }

    // ---- Dead-branch elimination ---------------------------------
    // Const-prop may have turned a `Var(cond)` reference into a
    // literal. If so, `Node::If { cond: LitU32(0)/false, .. }`
    // collapses to its `otherwise` body (and vice versa). Splices
    // the surviving branch into the parent scope so DCE sees a
    // flatter Program.
    t = std::time::Instant::now();
    let post_dbe = super::dead_branch::apply_dead_branch(&post_prop);
    if trace {
        eprintln!("[pl] dead_branch: {} us", t.elapsed().as_micros());
    }

    // ---- DCE on the post-arena Program ----------------------------
    t = std::time::Instant::now();
    let r = run_dce_resident(post_dbe, dispatcher).map_err(PipelineError::Dispatch);
    if trace {
        eprintln!("[pl] dce: {} us", t.elapsed().as_micros());
        eprintln!("[pl] === total: {} us ===", t_total.elapsed().as_micros());
    }
    r
}

fn alloc_many_lens(
    dispatcher: &dyn OptimizerDispatcher,
    byte_lens: &[usize],
) -> Result<Vec<u64>, PipelineError> {
    dispatcher
        .alloc_resident_many(byte_lens)
        .map_err(PipelineError::Dispatch)
}

fn run_dce_resident(
    program: Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<Program, DispatchError> {
    let encoded = encode_program(&program).map_err(|err| {
        DispatchError::Rejected(format!(
            "Fix: pipeline_resident DCE encoding failed: {err:?}"
        ))
    })?;
    let n = encoded.node_count;
    if n == 0 {
        return Ok(program);
    }

    // Allocate + upload PG buffers + frontier seed + RW state.
    let nodes_bytes = u32_slice_to_le_bytes(&encoded.nodes);
    let edge_offsets_bytes = u32_slice_to_le_bytes(&encoded.edge_offsets);
    let empty_edge_targets = [0u32];
    let edge_targets = if encoded.edge_targets.is_empty() {
        empty_edge_targets.as_slice()
    } else {
        encoded.edge_targets.as_slice()
    };
    let edge_targets_bytes = u32_slice_to_le_bytes(edge_targets);
    let empty_edge_kind = [0u32];
    let edge_kind = if encoded.edge_kind_mask.is_empty() {
        empty_edge_kind.as_slice()
    } else {
        encoded.edge_kind_mask.as_slice()
    };
    let edge_kind_bytes = u32_slice_to_le_bytes(edge_kind);
    let node_tags_bytes = u32_slice_to_le_bytes(&encoded.node_tags);

    let words = bitset_words(n) as usize;
    let mut seed = vec![0u32; words.max(1)];
    let root = ROOT_GRAPH_ID as usize;
    seed[root / 32] |= 1u32 << (root % 32);
    let seed_bytes = u32_slice_to_le_bytes(&seed);
    let frontier_out_bytes = vec![0u8; words.max(1) * 4];
    let changed_bytes = [0u8; 4];
    let dce_static_payloads: [&[u8]; 6] = [
        &nodes_bytes,
        &edge_offsets_bytes,
        &edge_targets_bytes,
        &edge_kind_bytes,
        &node_tags_bytes,
        &seed_bytes,
    ];
    let dce_static = acquire_static_uploads(
        dispatcher,
        RESIDENT_CACHE_DOMAIN_PIPELINE_DCE_RO,
        &dce_static_payloads,
        "pipeline_resident DCE read-only cache",
    )
    .map_err(|err| match err {
        PipelineError::Dispatch(dispatch) => dispatch,
        PipelineError::Encode(_) | PipelineError::LimitViolation(_) => DispatchError::BackendError(
            "Fix: DCE resident static upload surfaced a non-dispatch pipeline error.".to_string(),
        ),
    })?;
    let dce_mutable_payloads: [&[u8]; 2] = [&frontier_out_bytes, &changed_bytes];
    let dce_mutable_handles = alloc_many_d(dispatcher, &dce_mutable_payloads)?;
    let nodes_h = dce_static.handles[0];
    let edge_offsets_h = dce_static.handles[1];
    let edge_targets_h = dce_static.handles[2];
    let edge_kind_h = dce_static.handles[3];
    let node_tags_h = dce_static.handles[4];
    let frontier_in_h = dce_static.handles[5];
    let frontier_out_h = dce_mutable_handles[0];
    let changed_h = dce_mutable_handles[1];

    let shape = ProgramGraphShape::new(encoded.node_count, encoded.edge_count);
    // Optimizer-tailored DCE BFS: same buffer/binding layout as
    // `persistent_bfs`, but the kernel returns as soon as `changed`
    // reads zero after a step. For wide DAGs (diameter ≪ node_count)
    // this drops the persistent loop from O(node_count) iterations to
    // O(diameter). For chains the iteration count is unchanged.
    let analysis = build_dce_bfs_program(shape, n.max(1));

    // Same handle order as `persistent_bfs`: 4 ReadOnly PG buffers,
    // frontier_in (RO), frontier_out (RW), changed (RW). `wg_scratch`
    // is workgroup-only and not handed in.
    //
    // Grid: single workgroup. The csr-forward primitive serialises
    // the per-source loop on `local_x() == 0` so multiple workgroups
    // would just duplicate the BFS step; the persistent loop's
    // changed-flag reset would also race across workgroups, breaking
    // the early-exit. Single workgroup keeps semantics correct
    // regardless of `n`. Per-iter cost is O(n + e) on one warp of
    // the workgroup, which is acceptable up to n in the low 10000s.
    let dce_grid_x: u32 = 1;
    let mut frontier_out = Vec::with_capacity(words);
    let dce_step_handles = [
        nodes_h,
        edge_offsets_h,
        edge_targets_h,
        edge_kind_h,
        node_tags_h,
        frontier_in_h,
        frontier_out_h,
        changed_h,
    ];
    let dce_steps = [ResidentDispatchStep {
        program: &analysis,
        handle_ids: &dce_step_handles,
        grid_override: Some([dce_grid_x, 1, 1]),
    }];
    let dce_fills = [
        (frontier_out_h, frontier_out_bytes.len(), 0),
        (changed_h, changed_bytes.len(), 0),
    ];
    let mut byte_readbacks = Vec::with_capacity(1);
    let dce_result = upload_resident_sequence_read_u32_many_exact(
        dispatcher,
        &dce_fills,
        &[],
        &dce_steps,
        &[(frontier_out_h, words, "pipeline_resident DCE frontier_out")],
        &mut [&mut frontier_out],
        &mut byte_readbacks,
    );

    for h in [frontier_out_h, changed_h] {
        let _ = dispatcher.free_resident(h);
    }
    let dce_release = dispatcher.release_resident_static_uploads(dce_static);
    dce_result?;
    dce_release?;

    Ok(apply_live_bitset_mask(&program, &encoded, &frontier_out))
}

fn alloc_many_d(
    dispatcher: &dyn OptimizerDispatcher,
    payloads: &[&[u8]],
) -> Result<Vec<u64>, DispatchError> {
    let mut byte_lens = Vec::new();
    byte_lens.try_reserve(payloads.len()).map_err(|error| {
        DispatchError::BackendError(format!(
            "Fix: reserve DCE resident mutable byte lengths before allocation; requested {} payload(s): {error}.",
            payloads.len()
        ))
    })?;
    for payload in payloads {
        byte_lens.push(payload.len());
    }
    dispatcher.alloc_resident_many(&byte_lens)
}

fn acquire_static_uploads(
    dispatcher: &dyn OptimizerDispatcher,
    cache_domain: u64,
    payloads: &[&[u8]],
    context: &str,
) -> Result<ResidentStaticBufferSet, PipelineError> {
    let set = dispatcher
        .acquire_resident_static_uploads(cache_domain, payloads)
        .map_err(PipelineError::Dispatch)?;
    if set.handles.len() != payloads.len() {
        return Err(PipelineError::Dispatch(DispatchError::BackendError(
            format!(
                "Fix: {context} returned {} handle(s) for {} immutable payload(s).",
                set.handles.len(),
                payloads.len()
            ),
        )));
    }
    Ok(set)
}

#[cfg(test)]
fn read_resident_u32_exact(
    dispatcher: &dyn OptimizerDispatcher,
    handle: u64,
    expected_words: usize,
    context: &str,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let bytes = dispatcher.read_resident(handle)?;
    decode_u32_output_exact(&bytes, expected_words, context, out)
}

fn upload_resident_sequence_read_u32_many_exact(
    dispatcher: &dyn OptimizerDispatcher,
    fills: &[(u64, usize, u8)],
    uploads: &[(u64, &[u8])],
    steps: &[ResidentDispatchStep<'_>],
    requests: &[(u64, usize, &str)],
    outputs: &mut [&mut Vec<u32>],
    byte_outputs: &mut Vec<Vec<u8>>,
) -> Result<(), DispatchError> {
    if requests.len() != outputs.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: resident sequence readback expected matching request/output counts but got {} request(s) and {} output slot(s).",
            requests.len(),
            outputs.len()
        )));
    }
    let handles = requests
        .iter()
        .map(|(handle, _, _)| *handle)
        .collect::<Vec<_>>();
    dispatcher.fill_upload_resident_many_sequence_read_many_into(
        fills,
        uploads,
        steps,
        &handles,
        byte_outputs,
    )?;
    if byte_outputs.len() != requests.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: resident sequence readback returned {} byte output(s) for {} request(s).",
            byte_outputs.len(),
            requests.len()
        )));
    }
    for ((bytes, (_, expected_words, context)), out) in byte_outputs
        .iter()
        .zip(requests.iter())
        .zip(outputs.iter_mut())
    {
        decode_u32_output_exact(bytes, *expected_words, context, out)?;
    }
    Ok(())
}

fn upload_resident_sequence_read_u32_ranges_exact(
    dispatcher: &dyn OptimizerDispatcher,
    fills: &[(u64, usize, u8)],
    uploads: &[(u64, &[u8])],
    steps: &[ResidentDispatchStep<'_>],
    requests: &[(u64, usize, usize, &str)],
    outputs: &mut [&mut Vec<u32>],
    byte_outputs: &mut Vec<Vec<u8>>,
) -> Result<(), DispatchError> {
    if requests.len() != outputs.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: resident sequence range readback expected matching request/output counts but got {} request(s) and {} output slot(s).",
            requests.len(),
            outputs.len()
        )));
    }
    let mut ranges = Vec::new();
    ranges.try_reserve(requests.len()).map_err(|error| {
        DispatchError::BackendError(format!(
            "Fix: reserve resident sequence range readback descriptors for {} request(s): {error}.",
            requests.len()
        ))
    })?;
    for &(handle_id, byte_offset, expected_words, _) in requests {
        let byte_len = expected_words
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or_else(|| {
                DispatchError::BadInputs(format!(
                    "Fix: resident sequence range readback byte length overflows for handle {handle_id} word count {expected_words}."
                ))
            })?;
        ranges.push(ResidentReadRange {
            handle_id,
            byte_offset,
            byte_len,
        });
    }
    dispatcher.fill_upload_resident_many_sequence_read_ranges_into(
        fills,
        uploads,
        steps,
        &ranges,
        byte_outputs,
    )?;
    if byte_outputs.len() != requests.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: resident sequence range readback returned {} byte output(s) for {} request(s).",
            byte_outputs.len(),
            requests.len()
        )));
    }
    for ((bytes, (_, _, expected_words, context)), out) in byte_outputs
        .iter()
        .zip(requests.iter())
        .zip(outputs.iter_mut())
    {
        decode_u32_output_exact(bytes, *expected_words, context, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ReadbackDispatcher {
        bytes: Vec<u8>,
    }

    impl OptimizerDispatcher for ReadbackDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Err(DispatchError::Rejected(
                "Fix: readback test dispatcher only supports read_resident.".to_string(),
            ))
        }

        fn read_resident(&self, _handle: u64) -> Result<Vec<u8>, DispatchError> {
            Ok(self.bytes.clone())
        }
    }

    #[test]
    fn resident_readback_decodes_exact_u32s_into_reused_buffer() {
        let dispatcher = ReadbackDispatcher {
            bytes: u32_slice_to_le_bytes(&[3, 5]),
        };
        let mut out = Vec::with_capacity(4);
        let ptr = out.as_ptr();
        read_resident_u32_exact(&dispatcher, 7, 2, "resident test", &mut out)
            .expect("Fix: readback succeeds");
        assert_eq!(out, vec![3, 5]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn resident_readback_rejects_trailing_bytes() {
        let dispatcher = ReadbackDispatcher {
            bytes: vec![3, 0, 0, 0, 99],
        };
        let mut out = Vec::new();
        let err = read_resident_u32_exact(&dispatcher, 7, 1, "resident test", &mut out)
            .expect_err("trailing bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }
}
