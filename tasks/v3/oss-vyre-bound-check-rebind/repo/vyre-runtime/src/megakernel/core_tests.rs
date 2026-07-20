// Tests for `core.rs`. Split out per audit item #85 to keep the
// parent file focused on production code.

use super::*;
use crate::megakernel::{diffuse_priority_across_siblings, MegakernelLaunchPolicy};
use vyre_foundation::execution_plan::SchedulingPolicy;

#[test]
fn launch_geometry_pads_slots_and_caps_grid_by_workers() {
    let geometry = MegakernelLaunchGeometry::from_slots(300, 64, 256);
    assert_eq!(geometry.workgroup_size_x, 64);
    assert_eq!(geometry.slot_count, 320);
    assert_eq!(geometry.covering_worker_groups(), 5);
    assert_eq!(geometry.dispatch_grid, [5, 1, 1]);
}

#[test]
fn launch_geometry_preserves_legacy_worker_clamp() {
    let geometry = MegakernelLaunchGeometry::from_slots(1, 1_000, 256);
    assert_eq!(geometry.workgroup_size_x, 256);
    assert_eq!(geometry.slot_count, 256);
    assert_eq!(geometry.dispatch_grid, [1, 1, 1]);
}

#[test]
fn dispatch_grid_keeps_worker_count_as_ceiling() {
    let config = MegakernelConfig {
        worker_count: 2,
        ..MegakernelConfig::default()
    };
    assert_eq!(config.dispatch_grid(4096, 64), [2, 1, 1]);
}

#[test]
fn dispatch_grid_preserves_logical_queue_width_policy() {
    let config = MegakernelConfig {
        worker_count: 64,
        ..MegakernelConfig::default()
    };
    assert_eq!(config.dispatch_grid(300, 256), [2, 1, 1]);
}

#[test]
fn megakernel_helpers_delegate_to_shared_scheduling_policy() {
    let policy = SchedulingPolicy::standard();
    assert_eq!(
        MegakernelConfig::default().worker_count,
        policy.default_worker_count()
    );
    assert_eq!(
        worker_workgroup_size(1_000, 256),
        policy.worker_workgroup_size(1_000, 256)
    );
    assert_eq!(
        padded_slot_count(300, 64),
        policy.padded_slot_count(300, 64)
    );
    assert_eq!(
        dispatch_grid_for(64, 300, 256),
        policy.dispatch_grid_for(64, 300, 256)
    );
    assert_eq!(
        default_worker_groups_from_limits(65_536, 4_096),
        policy.default_worker_groups_from_limits(65_536, 4_096)
    );
}

#[test]
fn config_builds_launch_policy_from_continuation_task_queue() {
    let config = MegakernelConfig {
        worker_count: 64,
        expected_items_per_worker: 2,
        ..MegakernelConfig::default()
    };
    let item = MegakernelWorkItem {
        op_handle: 10,
        input_handle: 11,
        output_handle: 12,
        param: 13,
    };
    let ready = TaskWorkItem::from_work_item(1, 0, super::super::task::TaskPriority::Normal, item);
    let paused = ready.paused(20, 30, 40);
    let requeued = ready.requeued(50, 60, super::super::task::TaskPriority::High);

    let request = config
        .launch_request_for_tasks(&[ready, paused, requeued], 256, 65_536, 1_024)
        .expect("Fix: valid continuation tasks must produce a launch request");
    assert_eq!(request.queue_len, 2);
    assert_eq!(request.expected_hits_per_item, 2);
    assert_eq!(request.requeue_count, 2);
    assert_eq!(request.max_priority_age, 1);

    let rec = config
        .launch_recommendation_for_tasks(&[ready, paused, requeued], 256, 65_536, 1_024)
        .expect("Fix: valid continuation tasks must produce a launch recommendation");
    assert_eq!(rec.geometry.workgroup_size_x, 64);
    assert!(rec.age_priority_work);
}

#[test]
fn fusion_selection_into_matches_owned_selector() {
    let costs = [3.0, 1.0, 2.0, 4.0];
    let n = costs.len() as u32;
    let exchange_adj = vec![0; costs.len() * costs.len()];
    let owned = select_fused_subset(&costs, n, &exchange_adj);

    let mut scratch = FusionSelectionScratch::default();
    select_fused_subset_into(&costs, n, &exchange_adj, &mut scratch);

    assert_eq!(scratch.result(), owned.as_slice());
}

#[test]
fn fusion_compact_into_matches_owned_selector() {
    let costs = [30u16, 10, 20, 40];
    let n = costs.len() as u32;
    let exchange_adj = vec![0; costs.len() * costs.len()];
    let owned = select_fused_subset_compact(&costs, n, &exchange_adj);

    let mut scratch = FusionSelectionScratch::default();
    select_fused_subset_compact_into(&costs, n, &exchange_adj, &mut scratch);

    assert_eq!(scratch.result(), owned.as_slice());
}

#[test]
fn launch_policy_autotune_uses_local_min_cost_selection() {
    let policy = MegakernelLaunchPolicy::standard();
    assert_eq!(
        policy.autotune_hit_capacity_multiplier(&[1, 2, 4, 8], &[9.0, 5.0, 1.0, 3.0]),
        4
    );
    assert_eq!(
        policy.autotune_workgroup_size(&[32, 64, 128], &[7.0, 2.0, 3.0], 32),
        64
    );
}

#[test]
fn priority_diffusion_and_natural_gradient_are_runtime_local() {
    let diffused = diffuse_priority_across_siblings(&[10.0, 20.0], &[0.5, 0.25], 0.2, 1);
    assert_eq!(diffused, vec![9.0, 19.0]);

    let delta = MegakernelLaunchPolicy::natural_gradient_autotune_step(
        &[1.0, 0.0, 0.0, 1.0],
        &[3.0, -2.0],
        2,
        0.5,
    );
    assert_eq!(delta, vec![-1.5, 1.0]);
}

#[test]
fn natural_gradient_rejects_invalid_shape_before_output_growth() {
    let mut out = Vec::with_capacity(2);
    out.extend_from_slice(&[99.0, 100.0]);
    let ptr = out.as_ptr();

    MegakernelLaunchPolicy::natural_gradient_autotune_step_into(
        &[1.0, 0.0],
        &[3.0],
        2,
        0.5,
        &mut out,
    );

    assert!(out.is_empty());
    assert_eq!(out.capacity(), 2);
    assert_eq!(out.as_ptr(), ptr);
}
