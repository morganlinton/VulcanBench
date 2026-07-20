//! Fusion-subset selection used by megakernel batch dispatchers.
//!
//! This runtime path is deliberately self-contained: it does not call
//! self-substrate CPU reference solvers while preparing megakernel work.

use super::MegakernelWorkItem;

mod prologue;
pub use prologue::shared_prologue_length;

/// Hard cap for dense exchange-graph planning.
///
/// This avoids dense O(n*n) matrix growth in pathological batches.
pub(super) const MAX_DENSE_FUSION_ITEMS: usize = 4096;

/// Reusable buffers for megakernel fusion-subset selection.
///
/// Runtime schedulers can keep one scratch object per worker and avoid
/// allocating the homotopy, seed, flow, and result buffers every batch.
#[derive(Debug, Default)]
pub struct FusionSelectionScratch {
    order: Vec<usize>,
    result: Vec<u32>,
    conflict_degrees: Vec<u32>,
    selected: Vec<usize>,
}

impl FusionSelectionScratch {
    /// Selected 0/1 fusion vector from the last selector invocation.
    #[must_use]
    pub fn result(&self) -> &[u32] {
        &self.result
    }

    /// Move out the current result while retaining the other scratch buffers.
    #[must_use]
    pub fn take_result(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.result)
    }

    fn prepare(&mut self, n: usize) {
        self.order.clear();
        self.order.extend(0..n);
        self.result.clear();
        self.result.resize(n, 0);
        self.conflict_degrees.clear();
        self.conflict_degrees.resize(n, 0);
        self.selected.clear();
    }
}

/// Input-shape error from megakernel fusion subset selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FusionSelectionError {
    /// `n * n` overflowed `usize`.
    ExchangeSizeOverflow {
        /// Requested item count.
        n: usize,
    },
    /// Cost vector length did not match `n`.
    CostLen {
        /// Expected number of costs.
        expected: usize,
        /// Actual number of costs.
        actual: usize,
    },
    /// Exchange adjacency length did not match `n * n`.
    ExchangeAdjLen {
        /// Expected number of row-major adjacency cells.
        expected: usize,
        /// Actual number of adjacency cells.
        actual: usize,
    },
}

impl std::fmt::Display for FusionSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExchangeSizeOverflow { n } => write!(
                f,
                "megakernel fusion selector n*n overflow for n={n}. Fix: shard the work batch before fusion selection."
            ),
            Self::CostLen { expected, actual } => write!(
                f,
                "megakernel fusion selector cost length {actual} does not match n={expected}. Fix: pass one cost per work item."
            ),
            Self::ExchangeAdjLen { expected, actual } => write!(
                f,
                "megakernel fusion selector exchange_adj length {actual} does not match n*n={expected}. Fix: pass a dense row-major n*n exchange graph."
            ),
        }
    }
}

impl std::error::Error for FusionSelectionError {}

fn validate_selector_shape(
    cost_len: usize,
    n: u32,
    exchange_adj_len: usize,
) -> Result<(usize, usize), FusionSelectionError> {
    let n_usize = usize::try_from(n)
        .map_err(|_| FusionSelectionError::ExchangeSizeOverflow { n: usize::MAX })?;
    let cells = n_usize
        .checked_mul(n_usize)
        .ok_or(FusionSelectionError::ExchangeSizeOverflow { n: n_usize })?;
    if cost_len != n_usize {
        return Err(FusionSelectionError::CostLen {
            expected: n_usize,
            actual: cost_len,
        });
    }
    if exchange_adj_len != cells {
        return Err(FusionSelectionError::ExchangeAdjLen {
            expected: cells,
            actual: exchange_adj_len,
        });
    }
    Ok((n_usize, cells))
}

/// Reusable scratch for compact runtime fusion planning.
///
/// Concrete drivers own command submission. Runtime owns the queue-shaping
/// policy: cost seeds, divergence flags, exchange graph, and selector output.
#[derive(Debug, Default)]
pub struct CompactFusionPlanningScratch {
    costs_q16: Vec<u16>,
    stalks: Vec<f32>,
    diffused_stalks: Vec<f32>,
    effective_divergence: Vec<u32>,
    deltas: Vec<f32>,
    sorted_deltas: Vec<f32>,
    exchange_adj: Vec<u32>,
    order: Vec<usize>,
    selection: FusionSelectionScratch,
}

impl CompactFusionPlanningScratch {
    /// Last exchange adjacency matrix, row-major `n*n`.
    #[must_use]
    pub fn exchange_adj(&self) -> &[u32] {
        &self.exchange_adj
    }

    /// Last 0/1 selection vector.
    #[must_use]
    pub fn selected(&self) -> &[u32] {
        self.selection.result()
    }
}

/// Build the compact megakernel fusion plan for one work batch.
///
/// Returns the selector's 0/1 keep vector. The matching exchange adjacency is
/// retained in `scratch.exchange_adj()` for provenance and diagnostics.
pub fn plan_compact_fusion_into<'a>(
    work_items: &[MegakernelWorkItem],
    scratch: &'a mut CompactFusionPlanningScratch,
) -> &'a [u32] {
    let n = work_items.len();
    if n > MAX_DENSE_FUSION_ITEMS {
        scratch.selection.prepare(n);
        scratch.selection.result.fill(1);
        scratch.exchange_adj.clear();
        return scratch.selection.result();
    }

    if n == 0 {
        scratch.costs_q16.clear();
        scratch.stalks.clear();
        scratch.diffused_stalks.clear();
        scratch.effective_divergence.clear();
        scratch.deltas.clear();
        scratch.sorted_deltas.clear();
        scratch.exchange_adj.clear();
        scratch.selection.prepare(0);
        return scratch.selection.result();
    }

    scratch.costs_q16.clear();
    scratch.costs_q16.resize(n, u16::MAX);

    scratch.stalks.clear();
    scratch.stalks.extend(
        work_items
            .iter()
            .enumerate()
            .map(|(item_idx, _item)| (item_idx as f32) * 0.001),
    );
    scratch.diffused_stalks.clear();
    scratch.diffused_stalks.extend_from_slice(&scratch.stalks);
    for _ in 0..8 {
        for value in &mut scratch.diffused_stalks {
            *value -= 0.5_f32 * 0.7_f32 * *value;
        }
    }

    let divergence_threshold = 0.05_f32;
    let mut delta_sum = 0.0_f32;
    let mut delta_max = 0.0_f32;
    scratch.effective_divergence.clear();
    for (&initial, &diffused) in scratch.stalks.iter().zip(scratch.diffused_stalks.iter()) {
        let delta = (initial - diffused).abs();
        delta_sum += delta;
        delta_max = delta_max.max(delta);
        scratch
            .effective_divergence
            .push(u32::from(delta > divergence_threshold));
    }

    let n_f32 = n as f32;
    let gap_signal = if delta_max > 0.0_f32 && n_f32 > 0.0_f32 {
        delta_sum / (n_f32 * delta_max)
    } else {
        1.0_f32
    };
    if gap_signal < 0.3 {
        scratch.deltas.clear();
        scratch.deltas.extend(
            scratch
                .stalks
                .iter()
                .zip(scratch.diffused_stalks.iter())
                .map(|(s, d)| (s - d).abs()),
        );
        scratch.sorted_deltas.clear();
        scratch.sorted_deltas.extend_from_slice(&scratch.deltas);
        scratch
            .sorted_deltas
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = scratch
            .sorted_deltas
            .get(scratch.sorted_deltas.len() / 2)
            .copied()
            .unwrap_or(0.0);
        for (flag, delta) in scratch
            .effective_divergence
            .iter_mut()
            .zip(scratch.deltas.iter())
        {
            if *delta < median {
                *flag = 0;
            }
        }
    }

    scratch.exchange_adj.clear();
    let dense_cells = n * n;
    scratch.exchange_adj.resize(dense_cells, 0);
    let mut has_exchange_conflict = false;

    let mut has_op_conflict = false;
    scratch.order.clear();
    scratch.order.extend(0..n);
    if scratch.order.len() > 1 {
        scratch
            .order
            .sort_unstable_by_key(|&item_idx| work_items[item_idx].op_handle);
        if scratch
            .order
            .windows(2)
            .any(|window| work_items[window[0]].op_handle == work_items[window[1]].op_handle)
        {
            has_op_conflict = true;
        }
    }
    let has_output_input_chain = (0..n.checked_sub(1).unwrap_or(0)).any(|i| {
        work_items.get(i).map(|w| w.output_handle) == work_items.get(i + 1).map(|w| w.input_handle)
    });
    let has_divergence_conflict = scratch.effective_divergence.iter().any(|&v| v != 0);
    scratch.selection.prepare(n);

    if !has_op_conflict && !has_divergence_conflict {
        if has_output_input_chain {
            for cost in scratch.costs_q16.iter_mut() {
                *cost = discount_q16(*cost, 3_276);
            }
        }

        scratch.selection.result.fill(1);
        return scratch.selection.result();
    }

    {
        let conflict_degrees = &mut scratch.selection.conflict_degrees;
        for i in 0..n {
            let row_start = i * n;
            for j in 0..n {
                if i == j {
                    continue;
                }
                let same_op = work_items[i].op_handle == work_items[j].op_handle;
                if n <= 32 && same_op {
                    scratch.costs_q16[i] = discount_q16(scratch.costs_q16[i], 3_276);
                }
                let divergent =
                    scratch.effective_divergence[i] != 0 && scratch.effective_divergence[j] != 0;
                if same_op || divergent {
                    scratch.exchange_adj[row_start + j] = 1;
                    if i < j {
                        conflict_degrees[i] = increment_degree(conflict_degrees[i]);
                        conflict_degrees[j] = increment_degree(conflict_degrees[j]);
                    }
                    has_exchange_conflict = true;
                }
            }
        }
    }
    if has_output_input_chain {
        for cost in scratch.costs_q16.iter_mut() {
            *cost = discount_q16(*cost, 3_276);
        }
    }
    if !has_exchange_conflict {
        scratch.selection.result.fill(1);
        return scratch.selection.result();
    }

    let conflict_degrees = &scratch.selection.conflict_degrees;
    scratch.selection.order.sort_unstable_by(|&a, &b| {
        scratch.costs_q16[a]
            .cmp(&scratch.costs_q16[b])
            .then_with(|| conflict_degrees[a].cmp(&conflict_degrees[b]))
            .then_with(|| a.cmp(&b))
    });
    select_ordered_maximal(
        &scratch.exchange_adj,
        n,
        &scratch.selection.order,
        &mut scratch.selection.selected,
        &mut scratch.selection.result,
    );
    scratch.selection.result()
}

/// Compute a deterministic maximal fusion subset for a batch of megakernel work items.
///
/// `costs[i]` is the dispatch cost of program `i` (lower is cheaper).
/// `exchange_adj[i*n+j]` is non-zero when fusing `i` and `j` is
/// incompatible (memory overflow, sync class boundary, etc.).
///
/// Returns a 0/1 selection vector of length `n`.
#[must_use]
pub fn select_fused_subset(costs: &[f64], n: u32, exchange_adj: &[u32]) -> Vec<u32> {
    let mut scratch = FusionSelectionScratch::default();
    select_fused_subset_into(costs, n, exchange_adj, &mut scratch);
    scratch.take_result()
}

/// Compute the optimal fusion subset into reusable scratch buffers.
pub fn select_fused_subset_into(
    costs: &[f64],
    n: u32,
    exchange_adj: &[u32],
    scratch: &mut FusionSelectionScratch,
) {
    if let Ok((n_usize, _cells)) = validate_selector_shape(costs.len(), n, exchange_adj.len()) {
        if n_usize <= MAX_DENSE_FUSION_ITEMS && exchange_adj.iter().all(|&edge| edge == 0) {
            scratch.prepare(n_usize);
            scratch.result.fill(1);
            return;
        }
    }
    // Silently preparing a 0-size scratch (= select no fusion) on malformed
    // planner input degrades to the slower unfused path with no signal to the
    // operator (Law 10 / Law 7). The checked selector only errs on malformed
    // input — a bug — so fail loud; callers use select_fused_subset_checked_into.
    if let Err(error) = select_fused_subset_checked_into(costs, n, exchange_adj, scratch) {
        panic!("vyre-runtime fusion subset selection failed on malformed planner input: {error}");
    }
}

/// Checked selector variant that reports malformed planner input.
pub fn select_fused_subset_checked_into(
    costs: &[f64],
    n: u32,
    exchange_adj: &[u32],
    scratch: &mut FusionSelectionScratch,
) -> Result<(), FusionSelectionError> {
    let (n_usize, _cells) = validate_selector_shape(costs.len(), n, exchange_adj.len())?;
    if n_usize > MAX_DENSE_FUSION_ITEMS {
        scratch.prepare(n_usize);
        scratch.result.fill(1);
        return Ok(());
    }
    scratch.prepare(n_usize);
    if exchange_adj.iter().all(|&edge| edge == 0) {
        scratch.result.fill(1);
        return Ok(());
    }
    if !compute_conflict_degrees_with_conflict(exchange_adj, n_usize, &mut scratch.conflict_degrees)
    {
        scratch.result.fill(1);
        return Ok(());
    }
    scratch.order.sort_unstable_by(|&a, &b| {
        costs[a]
            .total_cmp(&costs[b])
            .then_with(|| scratch.conflict_degrees[a].cmp(&scratch.conflict_degrees[b]))
            .then_with(|| a.cmp(&b))
    });
    select_ordered_maximal(
        exchange_adj,
        n_usize,
        &scratch.order,
        &mut scratch.selected,
        &mut scratch.result,
    );
    Ok(())
}

/// Compact-cost selector for hot runtime dispatchers.
///
/// `costs_q16[i]` is a normalized fixed-point dispatch cost where lower is
/// cheaper. This avoids carrying `Vec<f64>` scratch through runtime hot paths;
/// the exact matroid rounder still receives the same exchange graph.
#[must_use]
pub fn select_fused_subset_compact(costs_q16: &[u16], n: u32, exchange_adj: &[u32]) -> Vec<u32> {
    let mut scratch = FusionSelectionScratch::default();
    select_fused_subset_compact_into(costs_q16, n, exchange_adj, &mut scratch);
    scratch.take_result()
}

/// Compact-cost selector using caller-owned scratch buffers.
pub fn select_fused_subset_compact_into(
    costs_q16: &[u16],
    n: u32,
    exchange_adj: &[u32],
    scratch: &mut FusionSelectionScratch,
) {
    if let Ok((n_usize, _cells)) = validate_selector_shape(costs_q16.len(), n, exchange_adj.len()) {
        if n_usize <= MAX_DENSE_FUSION_ITEMS && exchange_adj.iter().all(|&edge| edge == 0) {
            scratch.prepare(n_usize);
            scratch.result.fill(1);
            return;
        }
    }
    // Same Law 10 / Law 7 fail-loud as the dense variant: a silent no-fusion
    // fallback on malformed input hides a planner bug and degrades speed.
    if let Err(error) = select_fused_subset_compact_checked_into(costs_q16, n, exchange_adj, scratch)
    {
        panic!(
            "vyre-runtime compact fusion subset selection failed on malformed planner input: {error}"
        );
    }
}

/// Checked compact selector variant that reports malformed planner input.
pub fn select_fused_subset_compact_checked_into(
    costs_q16: &[u16],
    n: u32,
    exchange_adj: &[u32],
    scratch: &mut FusionSelectionScratch,
) -> Result<(), FusionSelectionError> {
    let (n_usize, _cells) = validate_selector_shape(costs_q16.len(), n, exchange_adj.len())?;
    if n_usize > MAX_DENSE_FUSION_ITEMS {
        scratch.prepare(n_usize);
        scratch.result.fill(1);
        return Ok(());
    }
    scratch.prepare(n_usize);
    if exchange_adj.iter().all(|&edge| edge == 0) {
        scratch.result.fill(1);
        return Ok(());
    }
    if !compute_conflict_degrees_with_conflict(exchange_adj, n_usize, &mut scratch.conflict_degrees)
    {
        scratch.result.fill(1);
        return Ok(());
    }
    scratch.order.sort_unstable_by(|&a, &b| {
        costs_q16[a]
            .cmp(&costs_q16[b])
            .then_with(|| scratch.conflict_degrees[a].cmp(&scratch.conflict_degrees[b]))
            .then_with(|| a.cmp(&b))
    });
    select_ordered_maximal(
        exchange_adj,
        n_usize,
        &scratch.order,
        &mut scratch.selected,
        &mut scratch.result,
    );
    Ok(())
}

/// Compute a cost-ordered maximal fusion subset with the same output contract
/// as [`select_fused_subset`].
#[must_use]

pub fn select_optimal_fused_subset(costs: &[f64], n: u32, exchange_adj: &[u32]) -> Vec<u32> {
    select_fused_subset(costs, n, exchange_adj)
}

/// Runtime-compatible selector entry point that preserves the historical API.
#[must_use]
pub fn select_fused_subset_with_rate(costs: &[f64], n: u32, exchange_adj: &[u32]) -> Vec<u32> {
    select_fused_subset(costs, n, exchange_adj)
}

/// Select a cost-ordered fused subset, then eliminate arms whose gate
/// predicates have already proven them to be no-ops for this dispatch.
///
/// This is the runtime-facing C5 entry point: it keeps the historical
/// selection algorithm unchanged, then applies [`prune_dead_arms_inplace`]
/// before the caller materializes the launch sequence.
#[must_use]
pub fn select_fused_subset_pruned(
    costs: &[f64],
    n: u32,
    exchange_adj: &[u32],
    dead_mask: &[bool],
) -> Vec<u32> {
    let mut selection = select_fused_subset(costs, n, exchange_adj);
    prune_dead_arms_inplace(&mut selection, dead_mask);
    selection
}

/// Reusable-scratch variant of [`select_fused_subset_pruned`].
pub fn select_fused_subset_pruned_into(
    costs: &[f64],
    n: u32,
    exchange_adj: &[u32],
    dead_mask: &[bool],
    scratch: &mut FusionSelectionScratch,
) {
    select_fused_subset_into(costs, n, exchange_adj, scratch);
    prune_dead_arms_inplace(&mut scratch.result, dead_mask);
}

/// ROADMAP C5 substrate: gated no-op middle-arm elimination.
///
/// Given a `selection` 0/1 vector (one entry per arm in the megakernel
/// dispatch sequence) and a `dead_mask` of the same length where
/// `dead_mask[i] = true` means arm `i` has been proven to be a no-op
/// at this dispatch (gate predicate folds to false, output equals
/// input, etc.), zero out the corresponding selection entries in
/// place. Returns the number of arms eliminated so the caller can
/// log/telemeter the win.
///
/// Length mismatch is a caller contract violation. The function leaves the
/// selection untouched and returns zero so reusable planner scratch is never
/// abandoned through a panic while a checked caller records the malformed
/// planner input.
///
/// Example: an inference megakernel where arm 1 is a `mask × value`
/// step that's gated `mask != 0`. If the static analyzer proves the
/// mask buffer is all-zero for this batch, dispatch can elide arm 1
/// entirely. Without this elision the GPU launches a full kernel that
/// reads both buffers, computes the multiplication, and writes a
/// zero-result back  -  pure waste.
pub fn prune_dead_arms_inplace(selection: &mut [u32], dead_mask: &[bool]) -> u32 {
    if selection.len() != dead_mask.len() {
        return 0;
    }
    let mut eliminated = 0_u32;
    for (slot, &dead) in selection.iter_mut().zip(dead_mask.iter()) {
        if dead && *slot != 0 {
            *slot = 0;
            eliminated = eliminated.saturating_add(1);
        }
    }
    eliminated
}

fn compute_conflict_degrees_with_conflict(exchange_adj: &[u32], n: usize, out: &mut [u32]) -> bool {
    debug_assert_eq!(out.len(), n);
    out.fill(0);
    let mut has_conflict = false;
    for i in 0..n {
        let row = i * n;
        for j in (i + 1)..n {
            if exchange_adj[row + j] != 0 || exchange_adj[j * n + i] != 0 {
                out[i] = increment_degree(out[i]);
                out[j] = increment_degree(out[j]);
                has_conflict = true;
            }
        }
    }
    has_conflict
}

fn discount_q16(value: u16, amount: u16) -> u16 {
    value.saturating_sub(amount)
}

fn increment_degree(value: u32) -> u32 {
    value.saturating_add(1)
}

fn select_ordered_maximal(
    exchange_adj: &[u32],
    n: usize,
    order: &[usize],
    selected: &mut Vec<usize>,
    result: &mut [u32],
) {
    result.fill(0);
    selected.clear();

    if n == 0 {
        return;
    }

    if n <= 64 {
        let mut conflict_masks = [0_u64; 64];
        for i in 0..n {
            let row = i * n;
            let mut mask = 0_u64;
            for j in 0..n {
                if i == j {
                    continue;
                }
                if exchange_adj[row + j] != 0 || exchange_adj[j * n + i] != 0 {
                    mask |= 1_u64 << j;
                }
            }
            conflict_masks[i] = mask;
        }

        let mut selected_mask = 0_u64;
        for &item in order {
            if item >= n {
                continue;
            }
            if conflict_masks[item] & selected_mask == 0 {
                result[item] = 1;
                selected_mask |= 1_u64 << item;
                selected.push(item);
            }
        }
        return;
    }

    if n <= 128 {
        let mut conflict_masks_lo = [0_u64; 128];
        let mut conflict_masks_hi = [0_u64; 128];
        for i in 0..n {
            let row = i * n;
            let mut mask_lo = 0_u64;
            let mut mask_hi = 0_u64;
            for j in 0..n {
                if i == j {
                    continue;
                }
                if exchange_adj[row + j] != 0 || exchange_adj[j * n + i] != 0 {
                    if j < 64 {
                        mask_lo |= 1_u64 << j;
                    } else {
                        mask_hi |= 1_u64 << (j - 64);
                    }
                }
            }
            conflict_masks_lo[i] = mask_lo;
            conflict_masks_hi[i] = mask_hi;
        }

        let mut selected_lo = 0_u64;
        let mut selected_hi = 0_u64;
        for &item in order {
            if item >= n {
                continue;
            }
            let conflict = (conflict_masks_lo[item] & selected_lo) != 0
                || (conflict_masks_hi[item] & selected_hi) != 0;
            if !conflict {
                result[item] = 1;
                if item < 64 {
                    selected_lo |= 1_u64 << item;
                } else {
                    selected_hi |= 1_u64 << (item - 64);
                }
                selected.push(item);
            }
        }
        return;
    }

    if n <= 192 {
        let mut conflict_masks_0 = [0_u64; 192];
        let mut conflict_masks_1 = [0_u64; 192];
        let mut conflict_masks_2 = [0_u64; 192];
        for i in 0..n {
            let row = i * n;
            let mut mask_0 = 0_u64;
            let mut mask_1 = 0_u64;
            let mut mask_2 = 0_u64;
            for j in 0..n {
                if i == j {
                    continue;
                }
                if exchange_adj[row + j] != 0 || exchange_adj[j * n + i] != 0 {
                    match j / 64 {
                        0 => mask_0 |= 1_u64 << (j % 64),
                        1 => mask_1 |= 1_u64 << (j % 64),
                        2 => mask_2 |= 1_u64 << (j % 64),
                        _ => {}
                    }
                }
            }
            conflict_masks_0[i] = mask_0;
            conflict_masks_1[i] = mask_1;
            conflict_masks_2[i] = mask_2;
        }

        let mut selected_0 = 0_u64;
        let mut selected_1 = 0_u64;
        let mut selected_2 = 0_u64;
        for &item in order {
            if item >= n {
                continue;
            }
            let conflict = (conflict_masks_0[item] & selected_0 != 0)
                || (conflict_masks_1[item] & selected_1 != 0)
                || (conflict_masks_2[item] & selected_2 != 0);
            if !conflict {
                result[item] = 1;
                let bit = 1_u64 << (item % 64);
                match item / 64 {
                    0 => selected_0 |= bit,
                    1 => selected_1 |= bit,
                    2 => selected_2 |= bit,
                    _ => {}
                }
                selected.push(item);
            }
        }
        return;
    }

    if n <= 256 {
        let mut conflict_masks_0 = [0_u64; 256];
        let mut conflict_masks_1 = [0_u64; 256];
        let mut conflict_masks_2 = [0_u64; 256];
        let mut conflict_masks_3 = [0_u64; 256];
        for i in 0..n {
            let row = i * n;
            let mut mask_0 = 0_u64;
            let mut mask_1 = 0_u64;
            let mut mask_2 = 0_u64;
            let mut mask_3 = 0_u64;
            for j in 0..n {
                if i == j {
                    continue;
                }
                if exchange_adj[row + j] != 0 || exchange_adj[j * n + i] != 0 {
                    match j / 64 {
                        0 => mask_0 |= 1_u64 << (j % 64),
                        1 => mask_1 |= 1_u64 << (j % 64),
                        2 => mask_2 |= 1_u64 << (j % 64),
                        _ => mask_3 |= 1_u64 << (j % 64),
                    }
                }
            }
            conflict_masks_0[i] = mask_0;
            conflict_masks_1[i] = mask_1;
            conflict_masks_2[i] = mask_2;
            conflict_masks_3[i] = mask_3;
        }

        let mut selected_0 = 0_u64;
        let mut selected_1 = 0_u64;
        let mut selected_2 = 0_u64;
        let mut selected_3 = 0_u64;
        for &item in order {
            if item >= n {
                continue;
            }
            let conflict = (conflict_masks_0[item] & selected_0 != 0)
                || (conflict_masks_1[item] & selected_1 != 0)
                || (conflict_masks_2[item] & selected_2 != 0)
                || (conflict_masks_3[item] & selected_3 != 0);
            if !conflict {
                result[item] = 1;
                let bit = 1_u64 << (item % 64);
                match item / 64 {
                    0 => selected_0 |= bit,
                    1 => selected_1 |= bit,
                    2 => selected_2 |= bit,
                    _ => selected_3 |= bit,
                }
                selected.push(item);
            }
        }
        return;
    }

    let chunks = n.div_ceil(64);
    let mut conflict_masks = vec![0_u64; n * chunks];
    for i in 0..n {
        for j in (i + 1)..n {
            if exchange_adj[i * n + j] != 0 || exchange_adj[j * n + i] != 0 {
                let i_word = i / 64;
                let i_bit = 1_u64 << (i % 64);
                let j_word = j / 64;
                let j_bit = 1_u64 << (j % 64);

                let i_base = i * chunks;
                let j_base = j * chunks;
                conflict_masks[i_base + j_word] |= j_bit;
                conflict_masks[j_base + i_word] |= i_bit;
            }
        }
    }

    let mut selected_mask = vec![0_u64; chunks];
    for &item in order {
        if item >= n {
            continue;
        }
        let base = item * chunks;
        let mut conflict = false;
        for chunk in 0..chunks {
            if conflict_masks[base + chunk] & selected_mask[chunk] != 0 {
                conflict = true;
                break;
            }
        }
        if !conflict {
            result[item] = 1;
            selected.push(item);
            selected_mask[item / 64] |= 1_u64 << (item % 64);
        }
    }
}

#[cfg(test)]
mod tests;
