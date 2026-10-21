//! Homotopy-based megakernel scheduling oracle.

#![allow(deprecated)]

use crate::cpu_references::{homotopy_euler_predictor_cpu, linear_homotopy_cpu};

/// Produce normalized fusion weights from dispatch costs.
///
/// Lower-cost entries receive higher target weight. The schedule starts
/// from a uniform distribution, evaluates a linear homotopy toward the
/// inverse-cost distribution, then advances `steps` Euler predictor
/// iterations by `dt`.
#[must_use]
pub fn schedule_via_homotopy(costs: &[f64], n: u32, steps: u32, dt: f64) -> Vec<f64> {
    let n_usize = n as usize;
    if n == 0 || costs.len() != n_usize {
        return Vec::new();
    }
    let uniform = vec![1.0 / f64::from(n); n_usize];
    let mut inverse: Vec<f64> = costs
        .iter()
        .map(|cost| 1.0 / cost.abs().max(1.0e-12))
        .collect();
    let total = inverse.iter().sum::<f64>().max(1.0e-12);
    for value in &mut inverse {
        *value /= total;
    }
    let t = (f64::from(steps) * dt).clamp(0.0, 1.0);
    let mut state = linear_homotopy_cpu(&uniform, &inverse, t);
    for _ in 0..steps {
        let velocity: Vec<f64> = inverse
            .iter()
            .zip(state.iter())
            .map(|(&target, &current)| target - current)
            .collect();
        state = homotopy_euler_predictor_cpu(&state, &velocity, dt.clamp(0.0, 1.0));
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_costs_produce_near_uniform_schedule() {
        let costs = vec![1.0, 1.0, 1.0, 1.0];
        let schedule = schedule_via_homotopy(&costs, 4, 10, 0.1);
        assert_eq!(schedule.len(), 4);
        // With uniform costs, all weights should be ≈ 0.25.
        for &w in &schedule {
            assert!(
                (w - 0.25).abs() < 0.1,
                "uniform costs should yield near-uniform weights, got {w}"
            );
        }
    }

    #[test]
    fn lower_cost_gets_higher_weight() {
        // Cost[0] = 1.0 (cheap), Cost[1] = 100.0 (expensive).
        let costs = vec![1.0, 100.0];
        let schedule = schedule_via_homotopy(&costs, 2, 20, 0.1);
        assert!(
            schedule[0] > schedule[1],
            "cheaper item should get higher weight: {:?}",
            schedule
        );
    }

    #[test]
    fn empty_costs_returns_empty() {
        let schedule = schedule_via_homotopy(&[], 0, 10, 0.1);
        assert!(schedule.is_empty());
    }

    #[test]
    fn weights_are_non_negative() {
        let costs = vec![0.5, 2.0, 10.0];
        let schedule = schedule_via_homotopy(&costs, 3, 5, 0.05);
        for &w in &schedule {
            assert!(w >= 0.0, "weights must be non-negative, got {w}");
        }
    }
}
