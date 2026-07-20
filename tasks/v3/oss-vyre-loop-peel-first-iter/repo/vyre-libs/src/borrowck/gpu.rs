//! GPU borrow checker: the NLL loan-liveness verdict computed on a device via
//! batched reachability, producing the identical [`Conflict`] set as the CPU
//! [`analyze`](super::analyze) engine.
//!
//! The CPU engine's two monotone bitset dataflows are equivalent to per-loan
//! reachability: a loan's issue reaches a point iff the point lies in the
//! forward closure of `{issue}` over the CFG; a use is reachable from a point
//! iff the point lies in the forward closure of the loan's uses over the
//! reversed CFG; the loan is live where both hold. Two loans of one place
//! conflict when one is live at the other's issue and at least one is mutable.
//!
//! [`analyze_crate_batched`] is the scale engine: it unions every function's
//! CFG into one *disconnected* graph (disjoint node ranges, so a closure can
//! never cross a function boundary) and runs every loan in the whole crate
//! through **two** device dispatches per shard: one forward-batch seeded at
//! every loan's issue, one backward-batch seeded at every loan's uses. Launch
//! overhead is amortized across shard groups, not per function and not per loan,
//! which is what makes a crate-scale borrow check GPU-fast. [`analyze_batched`]
//! is the single-function case. Both are backend-agnostic: the caller supplies
//! an [`OptimizerDispatcher`], which on the fleet is the CUDA backend.
//!
//! Memory scales as `total_loans * ceil(total_points / 32)` words; for very
//! large crates the function list is sharded into groups before dispatch.

use vyre_primitives::graph::persistent_bfs::PERSISTENT_BFS_WORKGROUP_SIZE;
use vyre_self_substrate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_self_substrate::persistent_bfs::{
    bfs_expand_resident_graph_batch_with_scratch_into, upload_resident_bfs_graph,
    PersistentBfsResidentScratch,
};

use super::{BorrowFacts, Conflict, ConflictKind, LoanKind};

/// All edge kinds traversed (the CFG is unlabeled for borrow liveness).
const ALLOW_ALL: u32 = 0xFFFF_FFFF;

fn bitset_words(n: u32) -> usize {
    (n as usize).div_ceil(32)
}

fn set_bit(words: &mut [u32], bit: u32) {
    words[(bit / 32) as usize] |= 1u32 << (bit % 32);
}

fn test_bit(words: &[u32], bit: u32) -> bool {
    (words[(bit / 32) as usize] >> (bit % 32)) & 1 == 1
}

/// Test `bit` in query `q`'s slice of a flattened `query_count * words` bitset.
fn slice_test_bit(flat: &[u32], words: usize, q: usize, bit: u32) -> bool {
    test_bit(&flat[q * words..(q + 1) * words], bit)
}

/// Build a CSR adjacency from CFG edges. `reverse` swaps each edge so a closure
/// walks predecessors instead of successors.
fn build_csr(n: u32, edges: &[(u32, u32)], reverse: bool) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let n = n as usize;
    let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); n];
    for &(a, b) in edges {
        let (from, to) = if reverse { (b, a) } else { (a, b) };
        if (from as usize) < n && (to as usize) < n {
            adjacency[from as usize].push(to);
        }
    }
    let mut offsets = Vec::with_capacity(n + 1);
    let mut targets = Vec::new();
    offsets.push(0u32);
    for successors in &adjacency {
        targets.extend_from_slice(successors);
        offsets.push(targets.len() as u32);
    }
    let masks = vec![1u32; targets.len()];
    (offsets, targets, masks)
}

/// Compute conflicting-borrow violations (rustc E0499 / E0502) for an entire
/// crate (a slice of per-function [`BorrowFacts`]) on a GPU device, using two
/// dispatches per shard regardless of function or loan count inside the shard.
///
/// Returns one `Vec<Conflict>` per input function, in input order; each is
/// exactly what the CPU [`analyze`](super::analyze) engine returns for that
/// function (loan indices are function-local, matching `analyze`).
///
/// # Errors
///
/// Returns [`DispatchError`] if a device upload or batch dispatch fails.
pub fn analyze_crate_batched(
    dispatcher: &dyn OptimizerDispatcher,
    functions: &[BorrowFacts],
) -> Result<Vec<Vec<Conflict>>, DispatchError> {
    analyze_crate_batched_with_shard_cap(dispatcher, functions, DEFAULT_SHARD_WORDS)
}

/// Per-shard seed/output buffer budget in u32 words. A shard's batch buffers are
/// `shard_loans * ceil(shard_points / 32)` words; functions are packed greedily
/// up to this many words so a crate of any size fits VRAM. ~32 MiB per buffer
/// at the default.
pub const DEFAULT_SHARD_WORDS: usize = 8 * 1024 * 1024;
const MAX_GRID_SYNC_FREE_SHARD_POINTS: u32 = PERSISTENT_BFS_WORKGROUP_SIZE[0];

/// Like [`analyze_crate_batched`] but with an explicit per-shard word budget.
/// Functions are packed into shards under `max_shard_words` and the persistent
/// BFS single-workgroup point limit (a single function always forms at least its
/// own shard); each shard runs in two dispatches and the per-function results
/// are concatenated in input order.
///
/// # Errors
///
/// Returns [`DispatchError`] if a device upload or batch dispatch fails.
pub fn analyze_crate_batched_with_shard_cap(
    dispatcher: &dyn OptimizerDispatcher,
    functions: &[BorrowFacts],
    max_shard_words: usize,
) -> Result<Vec<Vec<Conflict>>, DispatchError> {
    let mut results: Vec<Vec<Conflict>> = Vec::with_capacity(functions.len());
    let mut start = 0;
    while start < functions.len() {
        let mut end = start;
        let mut shard_points: u64 = 0;
        let mut shard_loans: u64 = 0;
        while end < functions.len() {
            let f = &functions[end];
            let next_points = shard_points + u64::from(f.point_count);
            let next_loans = shard_loans + f.loan_count() as u64;
            let next_words = next_points.div_ceil(32);
            let next_buffer = next_loans.saturating_mul(next_words);
            // Always take at least one function; otherwise close the shard
            // before exceeding the budget or forcing GridSync into PTX.
            if end > start
                && (next_buffer > max_shard_words as u64
                    || next_points > u64::from(MAX_GRID_SYNC_FREE_SHARD_POINTS))
            {
                break;
            }
            shard_points = next_points;
            shard_loans = next_loans;
            end += 1;
        }
        results.extend(batch_shard(dispatcher, &functions[start..end])?);
        start = end;
    }
    Ok(results)
}

/// One shard: union every function's CFG into a disconnected graph and run all
/// loans through two dispatches. Caller bounds the shard size; this is where the
/// device work happens.
fn batch_shard(
    dispatcher: &dyn OptimizerDispatcher,
    functions: &[BorrowFacts],
) -> Result<Vec<Vec<Conflict>>, DispatchError> {
    let empty = || functions.iter().map(|_| Vec::new()).collect::<Vec<_>>();

    // Disjoint node ranges: function fi occupies [point_base[fi], +point_count).
    let mut point_base = Vec::with_capacity(functions.len());
    let mut combined_points: u32 = 0;
    for f in functions {
        point_base.push(combined_points);
        combined_points = combined_points.saturating_add(f.point_count);
    }
    // Disjoint loan ranges in the flattened query set.
    let mut loan_base = Vec::with_capacity(functions.len());
    let mut total_loans: usize = 0;
    for f in functions {
        loan_base.push(total_loans);
        total_loans += f.loan_count();
    }
    if combined_points == 0 || total_loans == 0 {
        return Ok(empty());
    }
    let words = bitset_words(combined_points);
    // Functions are disconnected, so no closure exceeds its own function's
    // point count; the largest function bounds iterations (not the crate total).
    let max_iters = functions
        .iter()
        .map(|f| f.point_count)
        .max()
        .unwrap_or(1)
        .max(1);

    // Union every function's CFG into one disconnected graph (edges shifted into
    // each function's node range; no cross-function edges exist, so a closure
    // stays within its function).
    let mut edges: Vec<(u32, u32)> = Vec::new();
    for (fi, f) in functions.iter().enumerate() {
        let base = point_base[fi];
        for &(a, b) in &f.cfg_edges {
            edges.push((a + base, b + base));
        }
    }
    let (fwd_off, fwd_tgt, fwd_msk) = build_csr(combined_points, &edges, false);
    let (rev_off, rev_tgt, rev_msk) = build_csr(combined_points, &edges, true);
    let fwd_graph =
        upload_resident_bfs_graph(dispatcher, combined_points, &fwd_off, &fwd_tgt, &fwd_msk)?;
    let rev_graph =
        upload_resident_bfs_graph(dispatcher, combined_points, &rev_off, &rev_tgt, &rev_msk)?;

    // Per-loan seeds across the whole crate, flattened total_loans * words.
    let mut issue_seeds = vec![0u32; total_loans * words];
    let mut use_seeds = vec![0u32; total_loans * words];
    for (fi, f) in functions.iter().enumerate() {
        let pbase = point_base[fi];
        let lbase = loan_base[fi];
        for a in 0..f.loan_count() {
            // Mirror the CPU path: skip loans whose issue point is out of range.
            // An out-of-range issue point is a malformed BorrowFacts; the CPU
            // engine silently ignores it (its `if p < n` guard at issue_seed
            // construction).  Panic or silent bitset corruption here (bit index
            // >= words*32) would diverge from the CPU contract, so we match the
            // CPU's skip behaviour and report it via a debug assertion so the
            // caller can diagnose the bad facts.
            if f.loan_issued_at[a] >= f.point_count {
                debug_assert!(
                    false,
                    "BorrowFacts invariant violation: loan_issued_at[{a}]={} >= point_count={} \
                     in function {fi}; loan silently dropped from GPU issue seeds",
                    f.loan_issued_at[a],
                    f.point_count,
                );
                continue;
            }
            let g = lbase + a;
            set_bit(
                &mut issue_seeds[g * words..(g + 1) * words],
                pbase + f.loan_issued_at[a],
            );
        }
        for &(loan, point) in &f.loan_used_at {
            let a = loan as usize;
            if a < f.loan_count() {
                // Also guard the use-point against point_count, mirroring the CPU
                // `if (l as usize) < loans && (p as usize) < n` guard.
                if point >= f.point_count {
                    debug_assert!(
                        false,
                        "BorrowFacts invariant violation: loan_used_at point={point} >= \
                         point_count={} in function {fi}; use silently dropped from GPU use seeds",
                        f.point_count,
                    );
                    continue;
                }
                let g = lbase + a;
                set_bit(&mut use_seeds[g * words..(g + 1) * words], pbase + point);
            }
        }
    }

    let mut scratch = PersistentBfsResidentScratch::default();
    let mut forward = Vec::new();
    let mut forward_changed = Vec::new();
    bfs_expand_resident_graph_batch_with_scratch_into(
        dispatcher,
        &fwd_graph,
        &issue_seeds,
        total_loans,
        ALLOW_ALL,
        max_iters,
        &mut scratch,
        &mut forward,
        &mut forward_changed,
    )?;
    let mut backward = Vec::new();
    let mut backward_changed = Vec::new();
    bfs_expand_resident_graph_batch_with_scratch_into(
        dispatcher,
        &rev_graph,
        &use_seeds,
        total_loans,
        ALLOW_ALL,
        max_iters,
        &mut scratch,
        &mut backward,
        &mut backward_changed,
    )?;

    // Pair within each function (cross-function loans are never compared).
    let mut per_function = Vec::with_capacity(functions.len());
    for (fi, f) in functions.iter().enumerate() {
        let pbase = point_base[fi];
        let lbase = loan_base[fi];
        let loans = f.loan_count();
        let mut conflicts = Vec::new();
        for a in 0..loans {
            for b in (a + 1)..loans {
                if f.loan_place[a] != f.loan_place[b] {
                    continue;
                }
                let a_mut = f.loan_kind[a] == LoanKind::Mut;
                let b_mut = f.loan_kind[b] == LoanKind::Mut;
                if !(a_mut || b_mut) {
                    continue;
                }
                let issue_a = pbase + f.loan_issued_at[a];
                let issue_b = pbase + f.loan_issued_at[b];
                let ga = lbase + a;
                let gb = lbase + b;
                let a_live_at_b = slice_test_bit(&forward, words, ga, issue_b)
                    && slice_test_bit(&backward, words, ga, issue_b);
                let b_live_at_a = slice_test_bit(&forward, words, gb, issue_a)
                    && slice_test_bit(&backward, words, gb, issue_a);
                if a_live_at_b || b_live_at_a {
                    let (first, second) = if f.loan_issued_at[a] <= f.loan_issued_at[b] {
                        (a, b)
                    } else {
                        (b, a)
                    };
                    conflicts.push(Conflict {
                        first: first as u32,
                        second: second as u32,
                        offset: f.loan_offset[second],
                        kind: if a_mut && b_mut {
                            ConflictKind::TwoMutable
                        } else {
                            ConflictKind::MutableAndShared
                        },
                    });
                }
            }
        }
        per_function.push(conflicts);
    }
    Ok(per_function)
}

/// Compute borrow conflicts for ONE function on a GPU device, batched across
/// all its loans (two dispatches). The single-function case of
/// [`analyze_crate_batched`]; returns the same conflicts as the CPU
/// [`analyze`](super::analyze) engine.
///
/// # Errors
///
/// Returns [`DispatchError`] if a device upload or batch dispatch fails.
pub fn analyze_batched(
    dispatcher: &dyn OptimizerDispatcher,
    facts: &BorrowFacts,
) -> Result<Vec<Conflict>, DispatchError> {
    let mut per_function = analyze_crate_batched(dispatcher, std::slice::from_ref(facts))?;
    Ok(per_function.pop().unwrap_or_default())
}
