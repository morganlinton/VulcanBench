//! Submodular maximization  -  one greedy step.
//!
//! Submodular function maximization gives constant-factor approximation
//! guarantees ((1 - 1/e)-approximation for cardinality constraint per
//! Nemhauser-Wolsey-Fisher 1978). Recent work (Mirzasoleiman 2015
//! stochastic greedy, Buchbinder 2020 continuous extensions) makes it
//! GPU-friendly: in each iteration, evaluate marginal gain
//! `f(S ∪ {e}) - f(S)` for many candidate elements in parallel, then
//! pick the argmax.
//!
//! This file ships the **per-iteration argmax-of-marginals** primitive.
//! The full greedy loop is the caller's: dispatch this primitive K
//! times, each time updating S to include the picked element. The
//! candidate-evaluation `f(S ∪ {e})` is application-specific and
//! computed by the caller's Program.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::active_learning` | active-learning batch acquisition |
//! | future `vyre-libs::nn::coreset` | coreset construction for large datasets |
//! | future `vyre-libs::security::sensor_placement` | optimal IDS sensor placement |
//! | `vyre-driver` compile-cache eviction | pick K Programs to keep cached that maximize expected hit rate over recent dispatch trace. Same `argmax_of_marginals` primitive. |
//!
//! # Algorithm shape
//!
//! Inputs:
//!   - `gains[c]` for each candidate `c` in `0..n_candidates`: the
//!     pre-evaluated marginal gain f(S ∪ {c}) - f(S). Caller's
//!     responsibility to fill (typically by dispatching a separate
//!     evaluator Program over `n_candidates` lanes first).
//!   - `picked_mask[c]`: 1 iff `c` is already in S (excluded from
//!     argmax), 0 otherwise.
//!
//! Output:
//!   - `winner_idx[0]`: index of the candidate with maximum gain among
//!     unpicked, or u32::MAX if all are picked.
//!   - `winner_gain[0]`: the gain value.
//!
//! Implementation: lane 0 walks all candidates so tie-breaking and
//! exclusion-mask semantics remain deterministic across backends.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::submodular_argmax_of_marginals";

/// Sentinel value for "no winner found" (all candidates picked).
pub const NO_WINNER: u32 = u32::MAX;

/// Emit the argmax-of-marginals Program. Lane 0 walks `n_candidates`,
/// finds the max gain among unpicked candidates, writes (index, gain)
/// to the two single-element output buffers.
#[must_use]
pub fn argmax_of_marginals(
    gains: &str,
    picked_mask: &str,
    winner_idx: &str,
    winner_gain: &str,
    n_candidates: u32,
) -> Program {
    if n_candidates == 0 {
        return crate::invalid_output_program(
            OP_ID,
            winner_idx,
            DataType::U32,
            format!("Fix: argmax_of_marginals requires n_candidates > 0, got {n_candidates}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::eq(t.clone(), Expr::u32(0)),
        vec![
            Node::let_bind("best_idx", Expr::u32(NO_WINNER)),
            Node::let_bind("best_gain", Expr::u32(0)),
            Node::loop_for(
                "c",
                Expr::u32(0),
                Expr::u32(n_candidates),
                vec![Node::if_then(
                    Expr::eq(Expr::load(picked_mask, Expr::var("c")), Expr::u32(0)),
                    vec![
                        Node::let_bind("g", Expr::load(gains, Expr::var("c"))),
                        Node::if_then(
                            Expr::or(
                                Expr::eq(Expr::var("best_idx"), Expr::u32(NO_WINNER)),
                                Expr::gt(Expr::var("g"), Expr::var("best_gain")),
                            ),
                            vec![
                                Node::assign("best_idx", Expr::var("c")),
                                Node::assign("best_gain", Expr::var("g")),
                            ],
                        ),
                    ],
                )],
            ),
            Node::store(winner_idx, Expr::u32(0), Expr::var("best_idx")),
            Node::store(winner_gain, Expr::u32(0), Expr::var("best_gain")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(gains, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_candidates),
            BufferDecl::storage(picked_mask, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_candidates),
            BufferDecl::storage(winner_idx, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::storage(winner_gain, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference. Returns `(winner_idx, winner_gain)` with
/// `winner_idx == NO_WINNER` if all candidates picked.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn argmax_of_marginals_cpu(gains: &[u32], picked_mask: &[u32]) -> (u32, u32) {
    let mut best: Option<(u32, u32)> = None;
    for (i, (&g, &m)) in gains.iter().zip(picked_mask.iter()).enumerate() {
        if m != 0 {
            continue;
        }
        match best {
            None => best = Some((i as u32, g)),
            Some((_, bg)) if g > bg => best = Some((i as u32, g)),
            _ => {}
        }
    }
    best.unwrap_or((NO_WINNER, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_picks_global_max_when_nothing_picked() {
        let gains = vec![3, 7, 2, 9, 5];
        let picked = vec![0, 0, 0, 0, 0];
        let (idx, gain) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(idx, 3);
        assert_eq!(gain, 9);
    }

    #[test]
    fn cpu_skips_already_picked() {
        let gains = vec![3, 7, 2, 9, 5];
        let picked = vec![0, 0, 0, 1, 0]; // exclude index 3
        let (idx, gain) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(idx, 1);
        assert_eq!(gain, 7);
    }

    #[test]
    fn cpu_all_picked_returns_no_winner() {
        let gains = vec![1, 2, 3];
        let picked = vec![1, 1, 1];
        let (idx, gain) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(idx, NO_WINNER);
        assert_eq!(gain, 0);
    }

    #[test]
    fn cpu_mismatched_inputs_only_consider_complete_pairs() {
        let (idx, gain) = argmax_of_marginals_cpu(&[3, 9, 1], &[1, 0]);
        assert_eq!((idx, gain), (1, 9));
    }

    #[test]
    fn cpu_ties_pick_first() {
        let gains = vec![5, 5, 5];
        let picked = vec![0, 0, 0];
        let (idx, _) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(idx, 0);
    }

    #[test]
    fn cpu_simulated_greedy_loop_three_picks() {
        // Run three iterations, picking the next-best each time.
        let gains = vec![1, 5, 3, 8, 2];
        let mut picked = vec![0u32; gains.len()];

        let (i1, _) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(i1, 3); // gain 8
        picked[i1 as usize] = 1;

        let (i2, _) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(i2, 1); // gain 5
        picked[i2 as usize] = 1;

        let (i3, _) = argmax_of_marginals_cpu(&gains, &picked);
        assert_eq!(i3, 2); // gain 3
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = argmax_of_marginals("g", "p", "wi", "wg", 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["g", "p", "wi", "wg"]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 16);
        assert_eq!(p.buffers[2].count(), 1);
        assert_eq!(p.buffers[3].count(), 1);
    }

    #[test]
    fn zero_candidates_traps() {
        let p = argmax_of_marginals("g", "p", "wi", "wg", 0);
        assert!(p.stats().trap());
    }
}
