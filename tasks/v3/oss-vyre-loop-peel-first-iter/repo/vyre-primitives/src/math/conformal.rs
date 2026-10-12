//! Conformal prediction primitives  -  finite-sample, distribution-free
//! uncertainty intervals.
//!
//! Conformal prediction (Vovk 2005, Angelopoulos 2023) gives
//! prediction intervals with a guaranteed coverage probability  -  no
//! distributional assumptions on the data, no asymptotic arguments.
//! For split conformal:
//!
//! ```text
//!   1. Compute non-conformity scores s_i on a held-out calibration set.
//!   2. Take the ⌈(1 - α)(n + 1)⌉-th smallest s_i; call it q̂.
//!   3. For a new prediction ŷ: prediction interval = [ŷ - q̂, ŷ + q̂].
//! ```
//!
//! Coverage guarantee: P(y_new ∈ interval) ≥ 1 - α, exactly,
//! finite-sample, distribution-free.
//!
//! This file ships the **threshold** primitive  -  given calibration
//! scores already computed by the caller, find q̂. The
//! prediction-interval expansion is one elementwise add (no primitive
//! needed). Score computation is application-specific.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::uncertainty` | calibrated NN intervals without retraining |
//! | future `vyre-libs::observability::regression` | bounded-error performance regression detection |
//! | `vyre-driver` dispatch cost model | probabilistic circuits output intervals, not point estimates; conformal intervals on past dispatch latency feed megakernel scheduling as soft constraints |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::conformal_threshold";

/// Compute the conformal threshold q̂ given pre-sorted calibration
/// scores and target rank `k = ⌈(1 - α)(n + 1)⌉`. Single-lane primitive
///  -  lane 0 reads `scores[k - 1]` and writes it to `q_hat[0]`.
///
/// Note: callers must sort the scores BEFORE dispatching (compose with
/// any sort primitive). Unsorted input gives an arbitrary value.
#[must_use]
pub fn conformal_threshold(scores_sorted: &str, q_hat: &str, n: u32, k: u32) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            q_hat,
            DataType::U32,
            format!("Fix: conformal_threshold requires n > 0, got {n}."),
        );
    }
    if k == 0 || k > n {
        return crate::invalid_output_program(
            OP_ID,
            q_hat,
            DataType::U32,
            format!("Fix: conformal_threshold k must satisfy 1 <= k <= n, got k={k}, n={n}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::eq(t.clone(), Expr::u32(0)),
        vec![Node::store(
            q_hat,
            Expr::u32(0),
            Expr::load(scores_sorted, Expr::u32(k - 1)),
        )],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(scores_sorted, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(q_hat, 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Compute the conformal rank `k = ⌈(1 - α)(n + 1)⌉`. Host helper
/// because the calculation is non-vectorized and called once per
/// dispatch.
#[must_use]
pub fn conformal_rank(n: u32, alpha: f64) -> u32 {
    let Some(rank) = try_conformal_rank(n, alpha) else {
        return 0;
    };
    rank
}

/// Fallible conformal rank helper for callers that need explicit validation.
#[must_use]
pub fn try_conformal_rank(n: u32, alpha: f64) -> Option<u32> {
    if n == 0 || !(alpha > 0.0 && alpha < 1.0) {
        return None;
    }
    let raw = (1.0 - alpha) * (n as f64 + 1.0);
    let rank = raw.ceil() as u32;
    Some(rank.clamp(1, n))
}

/// CPU reference: given UNSORTED scores + target miscoverage α, return
/// the threshold value q̂ that the GPU would produce after a sort + the
/// `conformal_threshold` Program.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn conformal_threshold_cpu(scores: &[u32], alpha: f64) -> u32 {
    let n = scores.len() as u32;
    let Some(k) = try_conformal_rank(n, alpha) else {
        return 0;
    };
    let mut sorted = scores.to_vec();
    sorted.sort_unstable();
    sorted[(k - 1) as usize]
}

/// CPU reference: prediction interval `[y - q_hat, y + q_hat]`. Tiny
/// helper that pairs with the threshold; not a primitive (one
/// elementwise add saturates with arithmetic).
#[must_use]
pub fn predict_interval(y: u32, q_hat: u32) -> (u32, u32) {
    let lo = y.saturating_sub(q_hat);
    let hi = y.saturating_add(q_hat);
    (lo, hi)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            conformal_threshold("scores", "q_hat", 4, 2)
        },
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[10, 20, 30, 40]),
                crate::wire::pack_u32_slice(&[0]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[20])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_alpha_05_picks_median() {
        // n=9, α=0.5 → k = ⌈0.5 * 10⌉ = 5 (the 5th smallest).
        assert_eq!(conformal_rank(9, 0.5), 5);
    }

    #[test]
    fn rank_alpha_005_picks_high_quantile() {
        // n=99, α=0.05 → k = ⌈0.95 * 100⌉ = 95.
        assert_eq!(conformal_rank(99, 0.05), 95);
    }

    #[test]
    fn rank_clamps_to_n() {
        // Extreme alpha=1e-9, n=10 → k might compute > n; clamp to n.
        assert_eq!(conformal_rank(10, 1e-9), 10);
    }

    #[test]
    fn cpu_threshold_picks_correct_quantile() {
        let scores = vec![1, 5, 3, 8, 2, 9, 4, 7, 6];
        // sorted: 1, 2, 3, 4, 5, 6, 7, 8, 9
        // α=0.5, n=9, k=5 → sorted[4] = 5
        assert_eq!(conformal_threshold_cpu(&scores, 0.5), 5);
    }

    #[test]
    fn cpu_threshold_alpha_low_picks_high() {
        let scores = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        // α=0.1, n=10, k = ⌈0.9 * 11⌉ = 10 → sorted[9] = 10
        assert_eq!(conformal_threshold_cpu(&scores, 0.1), 10);
    }

    #[test]
    fn predict_interval_is_symmetric() {
        let (lo, hi) = predict_interval(10, 3);
        assert_eq!(lo, 7);
        assert_eq!(hi, 13);
    }

    #[test]
    fn predict_interval_saturates_low() {
        let (lo, hi) = predict_interval(2, 5);
        assert_eq!(lo, 0); // saturating sub
        assert_eq!(hi, 7);
    }

    #[test]
    fn predict_interval_saturates_high() {
        let (lo, hi) = predict_interval(u32::MAX, 5);
        assert_eq!(lo, u32::MAX - 5);
        assert_eq!(hi, u32::MAX); // saturating add
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = conformal_threshold("scores", "q", 100, 95);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["scores", "q"]);
        assert_eq!(p.buffers[0].count(), 100);
        assert_eq!(p.buffers[1].count(), 1);
    }

    #[test]
    fn k_zero_traps() {
        let p = conformal_threshold("s", "q", 100, 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn k_over_n_traps() {
        let p = conformal_threshold("s", "q", 10, 11);
        assert!(p.stats().trap());
    }
}
