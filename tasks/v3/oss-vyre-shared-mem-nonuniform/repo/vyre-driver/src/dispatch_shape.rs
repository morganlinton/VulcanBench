//! Backend-neutral dispatch shape comparison helpers.
//!
//! CUDA graph replay, pipeline cache reuse, and future backend command replay
//! all need the same answer to two questions: do these borrowed input batches
//! have the same byte shape, and does a runtime [`DispatchConfig`] preserve the
//! launch-relevant shape captured at compile time?

use crate::{fixpoint_iterations::resolve_fixpoint_iterations, DispatchConfig};

/// Return true when two borrowed input lists have the same arity and per-input
/// byte lengths.
#[must_use]
pub fn borrowed_input_shapes_match(left: &[&[u8]], right: &[&[u8]]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| left.len() == right.len())
}

/// Return true when every borrowed-input batch item has the same shape as the
/// first item.
#[must_use]
pub fn borrowed_input_batch_shapes_match(batches: &[&[&[u8]]]) -> bool {
    let Some((first, rest)) = batches.split_first() else {
        return true;
    };
    rest.iter()
        .all(|batch| borrowed_input_shapes_match(first, batch))
}

/// Return true when a runtime dispatch config preserves a compiled launch
/// shape.
#[must_use]
pub fn dispatch_configs_share_launch_shape(
    compiled: &DispatchConfig,
    runtime: &DispatchConfig,
) -> bool {
    compiled.profile == runtime.profile
        && ulp_budgets_share_launch_shape(compiled, runtime)
        && compiled.max_output_bytes == runtime.max_output_bytes
        && compiled.workgroup_override == runtime.workgroup_override
        && compiled.grid_override == runtime.grid_override
        && fixpoint_iterations_share_launch_shape(compiled, runtime)
        && compiled.speculation == runtime.speculation
        && compiled.persistent_thread == runtime.persistent_thread
        && compiled.cooperative == runtime.cooperative
}

fn fixpoint_iterations_share_launch_shape(
    compiled: &DispatchConfig,
    runtime: &DispatchConfig,
) -> bool {
    let Ok(compiled_iterations) = resolve_fixpoint_iterations(compiled, "dispatch-shape") else {
        return false;
    };
    let Ok(runtime_iterations) = resolve_fixpoint_iterations(runtime, "dispatch-shape") else {
        return false;
    };
    compiled_iterations == runtime_iterations
}

fn ulp_budgets_share_launch_shape(compiled: &DispatchConfig, runtime: &DispatchConfig) -> bool {
    compiled.ulp_budget.unwrap_or(0) == runtime.ulp_budget.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        borrowed_input_batch_shapes_match, borrowed_input_shapes_match,
        dispatch_configs_share_launch_shape,
    };
    use crate::DispatchConfig;

    #[test]
    fn borrowed_input_shapes_compare_arity_and_lengths_only() {
        let a0 = [1_u8, 2, 3];
        let a1 = [4_u8];
        let b0 = [9_u8, 8, 7];
        let b1 = [6_u8];
        let c = [5_u8, 4];

        assert!(borrowed_input_shapes_match(&[&a0, &a1], &[&b0, &b1]));
        assert!(!borrowed_input_shapes_match(&[&a0, &a1], &[&b0]));
        assert!(!borrowed_input_shapes_match(&[&a0, &a1], &[&b0, &c]));
    }

    #[test]
    fn borrowed_input_batch_shapes_accept_empty_and_uniform_batches() {
        let a0 = [1_u8, 2];
        let a1 = [3_u8, 4, 5];
        let b0 = [9_u8, 8];
        let b1 = [7_u8, 6, 5];
        let c1 = [0_u8];

        assert!(borrowed_input_batch_shapes_match(&[]));
        assert!(borrowed_input_batch_shapes_match(&[
            &[&a0, &a1],
            &[&b0, &b1]
        ]));
        assert!(!borrowed_input_batch_shapes_match(&[
            &[&a0, &a1],
            &[&b0, &c1]
        ]));
    }

    #[test]
    fn dispatch_config_launch_shape_ignores_timeout_but_tracks_launch_fields() {
        let base = DispatchConfig::default();
        let mut timeout_only = base.clone();
        timeout_only.timeout = Some(std::time::Duration::from_millis(1));
        assert!(dispatch_configs_share_launch_shape(&base, &timeout_only));

        let mut changed_grid = base.clone();
        changed_grid.grid_override = Some([2, 1, 1]);
        assert!(!dispatch_configs_share_launch_shape(&base, &changed_grid));

        let mut changed_fixpoint = base.clone();
        changed_fixpoint.fixpoint_iterations = Some(2);
        assert!(!dispatch_configs_share_launch_shape(
            &base,
            &changed_fixpoint
        ));
    }

    #[test]
    fn dispatch_config_launch_shape_canonicalizes_default_fixpoint_iteration() {
        let base = DispatchConfig::default();
        let mut explicit_one = base.clone();
        explicit_one.fixpoint_iterations = Some(1);

        assert!(
            dispatch_configs_share_launch_shape(&base, &explicit_one),
            "Fix: compiled pipelines must not miss cache/replay fast paths when runtime policy spells the default fixpoint iteration count explicitly."
        );
    }

    #[test]
    fn dispatch_config_launch_shape_rejects_invalid_zero_fixpoint_iteration() {
        let base = DispatchConfig::default();
        let mut explicit_zero = base.clone();
        explicit_zero.fixpoint_iterations = Some(0);

        assert!(
            !dispatch_configs_share_launch_shape(&base, &explicit_zero),
            "Fix: explicit zero fixpoint iterations are invalid policy and must not be treated as a compatible launch shape."
        );
    }

    #[test]
    fn dispatch_config_launch_shape_canonicalizes_strict_ulp_budget() {
        let base = DispatchConfig::default();
        let mut explicit_strict = base.clone();
        explicit_strict.ulp_budget = Some(0);

        assert!(
            dispatch_configs_share_launch_shape(&base, &explicit_strict),
            "Fix: strict ULP defaults should not force duplicate compiled dispatch shapes."
        );
    }

    #[test]
    fn dispatch_config_launch_shape_separates_relaxed_ulp_budget() {
        let base = DispatchConfig::default();
        let mut relaxed = base.clone();
        relaxed.ulp_budget = Some(1);

        assert!(
            !dispatch_configs_share_launch_shape(&base, &relaxed),
            "Fix: relaxed ULP budgets change target intrinsic policy and need distinct dispatch shapes."
        );
    }
}
