//! Planar non-overlap batching for optimizer rewrite candidates.
//!
//! This is the CPU-side scheduler contract used by [`ProgramPass::batch_apply`]
//! to turn many local rewrite candidates into a small number of disjoint
//! batches. The GPU primitive still lives in `vyre-primitives`; this module is
//! the backend-neutral contract that the foundation optimizer can use without
//! depending upward on primitive crates.

use std::sync::OnceLock;

/// Minimum candidate count before a pass should pay planning overhead.
///
/// Operators can tune this once per process with
/// `VYRE_PLANAR_REWRITE_BATCH_THRESHOLD=<usize>`. Invalid values fall back to
/// the release default.
#[must_use]
pub fn default_planar_rewrite_batch_threshold() -> usize {
    static VALUE: OnceLock<usize> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("VYRE_PLANAR_REWRITE_BATCH_THRESHOLD")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(8)
    })
}

/// One local rewrite candidate laid out on a 2D region grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewriteCandidate {
    /// Candidate row in the pass-defined planar layout.
    pub row: u32,
    /// Candidate column in the pass-defined planar layout.
    pub col: u32,
}

impl RewriteCandidate {
    /// Construct a rewrite candidate at `(row, col)`.
    #[must_use]
    pub const fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }
}

/// Rewrite candidates plus the grid geometry needed to plan disjoint batches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteBatchCandidates {
    candidates: Vec<RewriteCandidate>,
    height: u32,
    width: u32,
    footprint: u32,
    threshold: usize,
}

impl RewriteBatchCandidates {
    /// Empty candidate set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            candidates: Vec::new(),
            height: 0,
            width: 0,
            footprint: 1,
            threshold: default_planar_rewrite_batch_threshold(),
        }
    }

    /// Build a candidate set for a row-major `height x width` layout.
    #[must_use]
    pub fn new(candidates: Vec<RewriteCandidate>, height: u32, width: u32, footprint: u32) -> Self {
        Self {
            candidates,
            height,
            width,
            footprint,
            threshold: default_planar_rewrite_batch_threshold(),
        }
    }

    /// Override the candidate-count threshold that activates batching.
    #[must_use]
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Number of candidate rewrites.
    #[must_use]
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Whether there are no candidate rewrites.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Candidate-count threshold used by [`Self::should_batch`].
    #[must_use]
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Return true when the pass should use planar batching instead of its
    /// sequential fallback.
    #[must_use]
    pub fn should_batch(&self) -> bool {
        self.candidates.len() > self.threshold
            && self.height != 0
            && self.width != 0
            && self.footprint != 0
    }

    /// Produce a complete multi-wave batch plan.
    #[must_use]
    pub fn plan(&self) -> RewriteBatchPlan {
        if self.candidates.is_empty() || self.height == 0 || self.width == 0 || self.footprint == 0
        {
            return RewriteBatchPlan::empty(self.candidates.len(), self.threshold);
        }
        let Some(cells) = cell_count(self.height, self.width) else {
            return RewriteBatchPlan::empty(self.candidates.len(), self.threshold);
        };

        let mut remaining = vec![false; self.candidates.len()];
        let mut remaining_count = 0usize;
        for (index, candidate) in self.candidates.iter().enumerate() {
            if candidate_index(*candidate, self.height, self.width, cells).is_some() {
                remaining[index] = true;
                remaining_count += 1;
            }
        }

        let mut batches = Vec::new();
        while remaining_count != 0 {
            let mut mask = vec![0u32; cells];
            for (index, candidate) in self.candidates.iter().enumerate() {
                if !remaining[index] {
                    continue;
                }
                if let Some(cell) = candidate_index(*candidate, self.height, self.width, cells) {
                    mask[cell] = 1;
                }
            }

            let chosen =
                planar_rewrite_schedule_mask(&mask, self.height, self.width, self.footprint);
            let mut items = Vec::new();
            for (index, candidate) in self.candidates.iter().enumerate() {
                if !remaining[index] {
                    continue;
                }
                let Some(cell) = candidate_index(*candidate, self.height, self.width, cells) else {
                    continue;
                };
                if chosen.get(cell).copied().unwrap_or(0) != 0 {
                    remaining[index] = false;
                    remaining_count -= 1;
                    items.push(RewriteBatchItem {
                        candidate_index: index,
                        row: candidate.row,
                        col: candidate.col,
                    });
                }
            }

            if items.is_empty() {
                return RewriteBatchPlan::empty(self.candidates.len(), self.threshold);
            }
            batches.push(RewriteBatch { items });
        }

        RewriteBatchPlan {
            batches,
            candidate_count: self.candidates.len(),
            threshold: self.threshold,
        }
    }
}

/// A selected rewrite candidate inside one disjoint batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewriteBatchItem {
    /// Index into the original candidate vector.
    pub candidate_index: usize,
    /// Candidate row in the pass-defined planar layout.
    pub row: u32,
    /// Candidate column in the pass-defined planar layout.
    pub col: u32,
}

/// One wave of mutually non-overlapping rewrite candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteBatch {
    items: Vec<RewriteBatchItem>,
}

impl RewriteBatch {
    /// Candidates selected for this wave.
    #[must_use]
    pub fn items(&self) -> &[RewriteBatchItem] {
        &self.items
    }

    /// Number of rewrites in this wave.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether this batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Complete multi-wave rewrite batch plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteBatchPlan {
    batches: Vec<RewriteBatch>,
    candidate_count: usize,
    threshold: usize,
}

impl RewriteBatchPlan {
    /// Empty plan preserving source candidate metadata.
    #[must_use]
    pub const fn empty(candidate_count: usize, threshold: usize) -> Self {
        Self {
            batches: Vec::new(),
            candidate_count,
            threshold,
        }
    }

    /// Disjoint rewrite waves.
    #[must_use]
    pub fn batches(&self) -> &[RewriteBatch] {
        &self.batches
    }

    /// Total source candidates considered.
    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    /// Candidate-count threshold used to build this plan.
    #[must_use]
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Number of planned waves.
    #[must_use]
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    /// Return true when the plan selected at least one rewrite.
    #[must_use]
    pub fn has_batches(&self) -> bool {
        !self.batches.is_empty()
    }
}

/// Greedy planar non-overlap selection for a single wave.
#[must_use]
pub fn planar_rewrite_schedule_mask(
    candidates: &[u32],
    height: u32,
    width: u32,
    footprint: u32,
) -> Vec<u32> {
    if height == 0 || width == 0 || footprint == 0 {
        return Vec::new();
    }
    let Some(cells) = cell_count(height, width) else {
        return Vec::new();
    };
    let h = height as usize;
    let w = width as usize;
    let k = footprint as usize;
    let mut chosen = vec![0u32; cells];

    for row in 0..h {
        for col in 0..w {
            let addr = row * w + col;
            if candidates.get(addr).copied().unwrap_or(0) == 0 {
                continue;
            }
            let mut conflict = false;
            for d_row in 0..k {
                for d_col in 0..k {
                    if d_row > row || d_col > col {
                        continue;
                    }
                    if chosen[(row - d_row) * w + (col - d_col)] != 0 {
                        conflict = true;
                        break;
                    }
                }
                if conflict {
                    break;
                }
            }
            if !conflict {
                chosen[addr] = 1;
            }
        }
    }

    chosen
}

fn candidate_index(
    candidate: RewriteCandidate,
    height: u32,
    width: u32,
    cells: usize,
) -> Option<usize> {
    let row = candidate.row as usize;
    let col = candidate.col as usize;
    let height = height as usize;
    let width = width as usize;
    if row >= height || col >= width {
        return None;
    }
    let index = row.checked_mul(width)?.checked_add(col)?;
    (index < cells).then_some(index)
}

fn cell_count(height: u32, width: u32) -> Option<usize> {
    let cells = height.checked_mul(width)?;
    usize::try_from(cells).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_scheduler_selects_disjoint_wave() {
        let schedule = planar_rewrite_schedule_mask(&vec![1; 16], 4, 4, 2);
        assert_eq!(schedule.iter().sum::<u32>(), 4);
        assert_eq!(schedule[0], 1);
        assert_eq!(schedule[2], 1);
        assert_eq!(schedule[8], 1);
        assert_eq!(schedule[10], 1);
    }

    #[test]
    fn multi_wave_plan_covers_every_candidate_once() {
        let candidates = (0..6)
            .map(|col| RewriteCandidate::new(0, col))
            .collect::<Vec<_>>();
        let plan = RewriteBatchCandidates::new(candidates, 1, 6, 2)
            .with_threshold(1)
            .plan();

        assert_eq!(plan.candidate_count(), 6);
        assert_eq!(plan.batch_count(), 2);
        assert_eq!(plan.batches()[0].items().len(), 3);
        assert_eq!(plan.batches()[1].items().len(), 3);

        let mut seen = vec![false; 6];
        for batch in plan.batches() {
            for item in batch.items() {
                assert!(!seen[item.candidate_index]);
                seen[item.candidate_index] = true;
            }
        }
        assert!(seen.into_iter().all(|v| v));
    }

    #[test]
    fn threshold_gate_is_strictly_greater_than_threshold() {
        let candidates = vec![
            RewriteCandidate::new(0, 0),
            RewriteCandidate::new(0, 1),
            RewriteCandidate::new(0, 2),
        ];
        assert!(!RewriteBatchCandidates::new(candidates.clone(), 1, 3, 1)
            .with_threshold(3)
            .should_batch());
        assert!(RewriteBatchCandidates::new(candidates, 1, 3, 1)
            .with_threshold(2)
            .should_batch());
    }
}
