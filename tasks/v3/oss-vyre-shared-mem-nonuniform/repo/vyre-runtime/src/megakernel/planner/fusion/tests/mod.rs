use super::*;

fn item(op_handle: u32, input_handle: u32, output_handle: u32) -> MegakernelWorkItem {
    MegakernelWorkItem {
        op_handle,
        input_handle,
        output_handle,
        param: 0,
    }
}

#[test]
fn compact_plan_reuses_scratch_and_records_exchange_graph() {
    let work = [item(7, 1, 2), item(7, 3, 4), item(9, 4, 5)];
    let mut scratch = CompactFusionPlanningScratch::default();

    let selected = plan_compact_fusion_into(&work, &mut scratch).to_vec();

    assert_eq!(selected.len(), work.len());
    assert_eq!(scratch.exchange_adj().len(), work.len() * work.len());
    assert_eq!(
        scratch.exchange_adj()[1],
        1,
        "same-op work items must be connected in the runtime exchange graph"
    );
    assert_eq!(
        scratch.exchange_adj()[5],
        0,
        "linear output->input discount changes cost, not exchange incompatibility"
    );
}

#[test]
fn compact_plan_empty_batch_clears_previous_scratch() {
    let work = [item(1, 1, 2)];
    let mut scratch = CompactFusionPlanningScratch::default();
    let selected_before_clear = plan_compact_fusion_into(&work, &mut scratch);
    assert_eq!(selected_before_clear.len(), work.len());
    assert!(!scratch.exchange_adj().is_empty());

    let selected = plan_compact_fusion_into(&[], &mut scratch);

    assert!(selected.is_empty());
    assert!(scratch.exchange_adj().is_empty());
}

#[test]
fn compact_plan_no_conflict_fast_path_selects_all_without_selector_pass() {
    let work = [item(1, 10, 20), item(2, 30, 40), item(3, 50, 60)];
    let mut scratch = CompactFusionPlanningScratch::default();

    let selected = plan_compact_fusion_into(&work, &mut scratch).to_vec();

    assert_eq!(selected, vec![1, 1, 1]);
    assert_eq!(scratch.exchange_adj().len(), work.len() * work.len());
    assert!(
        scratch.exchange_adj().iter().all(|&edge| edge == 0),
        "no-conflict compact planning must keep a zero exchange graph for diagnostics"
    );
}

#[test]
fn selector_conflict_free_returns_all_selected() {
    let costs = [3.0_f64, 1.0, 2.0];
    let exchange_adj = vec![0_u32; 9];

    let selected = select_fused_subset(&costs, 3, &exchange_adj);
    let selected_with_rate = select_fused_subset_with_rate(&costs, 3, &exchange_adj);
    let selected_pruned = select_fused_subset_pruned(&costs, 3, &exchange_adj, &[false; 3]);

    assert_eq!(selected, vec![1, 1, 1]);
    assert_eq!(selected_with_rate, vec![1, 1, 1]);
    assert_eq!(selected_pruned, vec![1, 1, 1]);
}

#[test]
fn selector_compact_conflict_free_returns_all_ones() {
    let costs = [5_u16, 4, 3, 2];
    let exchange_adj = vec![0_u32; 16];

    let selected = select_fused_subset_compact(&costs, 4, &exchange_adj);

    assert_eq!(selected, vec![1, 1, 1, 1]);
}

#[test]
fn selector_compact_conflict_free_large_n_uses_mask_fast_path() {
    let n = 70_usize;
    let costs: Vec<u16> = (0..n).map(|idx| idx as u16).collect();
    let exchange_adj = vec![0_u32; n * n];

    let selected = select_fused_subset_compact(&costs, n as u32, &exchange_adj);

    assert_eq!(selected.len(), n);
    assert!(selected.iter().all(|&v| v == 1));
}

#[test]
fn selector_conflict_free_large_n_uses_mask_fast_path() {
    let n = 130_usize;
    let costs: Vec<f64> = (0..n)
        .map(|idx| {
            let as_u = idx as u32;
            (as_u as f64) + 0.1_f64 * (as_u as f64 % 17_u32 as f64)
        })
        .collect();
    let exchange_adj = vec![0_u32; n * n];

    let selected = select_fused_subset(&costs, n as u32, &exchange_adj);

    assert_eq!(selected.len(), n);
    assert!(selected.iter().all(|&v| v == 1));
}

#[test]
fn selector_and_compact_conflict_free_multisize_fast_paths_cover_mid_sizes() {
    for n in 65_usize..=130_usize {
        let costs_u16: Vec<u16> = (0..n).rev().map(|idx| idx as u16).collect();
        let exchange_adj = vec![0_u32; n * n];
        let compact = select_fused_subset_compact(&costs_u16, n as u32, &exchange_adj);
        assert_eq!(compact.len(), n);
        assert!(compact.iter().all(|&v| v == 1));

        let costs_f64: Vec<f64> = (0..n).rev().map(|idx| idx as f64).collect();
        let selected = select_fused_subset(&costs_f64, n as u32, &exchange_adj);
        assert_eq!(selected.len(), n);
        assert!(selected.iter().all(|&v| v == 1));
    }
}

#[test]
fn selector_and_compact_conflict_free_bitset_boundaries() {
    for &n in &[64_usize, 65, 128, 129, 192, 193, 256] {
        let costs_u16: Vec<u16> = (0..n).rev().map(|idx| idx as u16 + 1).collect();
        let exchange_adj = vec![0_u32; n * n];
        let compact = select_fused_subset_compact(&costs_u16, n as u32, &exchange_adj);
        assert_eq!(compact.len(), n);
        assert!(
            compact.iter().all(|&v| v == 1),
            "compact boundary {} should be all ones",
            n
        );

        let costs_f64: Vec<f64> = (0..n).rev().map(|idx| idx as f64 + 0.5).collect();
        let selected = select_fused_subset(&costs_f64, n as u32, &exchange_adj);
        assert_eq!(selected.len(), n);
        assert!(
            selected.iter().all(|&v| v == 1),
            "selector boundary {} should be all ones",
            n
        );
    }
}

#[test]
fn selector_large_n_with_forced_conflict_skips_one_selected_arm() {
    for n in [257_usize, 300_usize] {
        let costs_u16: Vec<u16> = (0..n).map(|idx| idx as u16).collect();
        let costs_f64: Vec<f64> = (0..n).map(|idx| idx as f64).collect();
        let mut exchange_adj = vec![0_u32; n * n];
        let a1 = 255_usize;
        let b1 = 256_usize.min(n - 1);
        let a2 = 0_usize;
        let b2 = 1_usize;
        exchange_adj[a1 * n + b1] = 1;
        exchange_adj[b1 * n + a1] = 1;
        exchange_adj[a2 * n + b2] = 1;
        exchange_adj[b2 * n + a2] = 1;

        let compact = select_fused_subset_compact(&costs_u16, n as u32, &exchange_adj);
        assert_eq!(compact.len(), n);
        assert_eq!(compact[a1], 1);
        assert_eq!(compact[b1], 0);
        assert_eq!(compact[a2], 1);
        assert_eq!(compact[b2], 0);
        assert_eq!(compact.iter().sum::<u32>(), (n as u32) - 2);

        let selected = select_fused_subset(&costs_f64, n as u32, &exchange_adj);
        assert_eq!(selected.len(), n);
        assert_eq!(selected[a1], 1);
        assert_eq!(selected[b1], 0);
        assert_eq!(selected[a2], 1);
        assert_eq!(selected[b2], 0);
        assert_eq!(selected.iter().sum::<u32>(), (n as u32) - 2);
    }
}

#[test]
fn selector_large_n_without_conflict_returns_all_selected() {
    for n in [257_usize, 300_usize] {
        let costs_u16: Vec<u16> = (0..n).map(|idx| (idx * 13 % 2048) as u16).collect();
        let costs_f64: Vec<f64> = (0..n).map(|idx| idx as f64 + (idx % 19) as f64).collect();
        let exchange_adj = vec![0_u32; n * n];

        let compact = select_fused_subset_compact(&costs_u16, n as u32, &exchange_adj);
        assert_eq!(compact.len(), n);
        assert!(compact.iter().all(|&v| v == 1));

        let selected = select_fused_subset(&costs_f64, n as u32, &exchange_adj);
        assert_eq!(selected.len(), n);
        assert!(selected.iter().all(|&v| v == 1));
    }
}

#[test]
fn selector_large_n_with_asymmetric_conflict_treats_as_conflict() {
    let n = 257_usize;
    let a = 255_usize;
    let b = 1_usize;
    let costs_u16: Vec<u16> = (0..n)
        .map(|idx| {
            if idx == a {
                1
            } else if idx == b {
                2
            } else {
                10
            }
        })
        .collect();
    let costs_f64: Vec<f64> = (0..n)
        .map(|idx| {
            if idx == a {
                0.0_f64
            } else if idx == b {
                1000.0_f64
            } else {
                (idx % 11) as f64 + 0.125
            }
        })
        .collect();
    let mut exchange_adj = vec![0_u32; n * n];
    exchange_adj[a * n + b] = 1;

    let compact = select_fused_subset_compact(&costs_u16, n as u32, &exchange_adj);
    assert_eq!(compact.len(), n);
    assert_eq!(compact[a], 1);
    assert_eq!(compact[b], 0);

    let selected = select_fused_subset(&costs_f64, n as u32, &exchange_adj);
    assert_eq!(selected.len(), n);
    assert_eq!(selected[a], 1);
    assert_eq!(selected[b], 0);
}

#[test]
fn selector_compact_keeps_low_cost_compatible_items() {
    let costs = [1_u16, 2, 3, 4];
    let mut exchange_adj = vec![0_u32; 16];
    exchange_adj[1] = 1;
    exchange_adj[4] = 1;

    let mut scratch = FusionSelectionScratch::default();
    let err = select_fused_subset_compact_checked_into(&costs, 4, &exchange_adj, &mut scratch);
    assert!(err.is_ok());
    assert_eq!(scratch.result, vec![1, 0, 1, 1]);
    assert_eq!(scratch.selected, vec![0, 2, 3]);
}

// ── C5: gated no-op middle-arm elimination tests ────────────────

#[test]
fn prune_dead_arms_zeroes_only_selected_dead_arms() {
    let mut sel = vec![1, 1, 1, 1, 1];
    let dead = vec![false, true, false, true, false];
    let n = prune_dead_arms_inplace(&mut sel, &dead);
    assert_eq!(sel, vec![1, 0, 1, 0, 1]);
    assert_eq!(n, 2);
}

#[test]
fn select_fused_subset_pruned_eliminates_dead_selected_arm() {
    let costs = [1.0, 2.0, 3.0];
    let exchange_adj = vec![0_u32; 9];
    let dead = [false, true, false];

    let selected = select_fused_subset_pruned(&costs, 3, &exchange_adj, &dead);

    assert_eq!(selected, vec![1, 0, 1]);
}

#[test]
fn select_fused_subset_pruned_into_reuses_selection_scratch() {
    let costs = [1.0, 2.0, 3.0, 4.0];
    let exchange_adj = vec![0_u32; 16];
    let dead = [true, false, true, false];
    let mut scratch = FusionSelectionScratch::default();

    select_fused_subset_pruned_into(&costs, 4, &exchange_adj, &dead, &mut scratch);

    assert_eq!(scratch.result, vec![0, 1, 0, 1]);
    assert_eq!(scratch.order.len(), 4);
}

#[test]
fn prune_dead_arms_does_not_count_unselected_dead_arms() {
    // Arm 1 is dead but ALREADY unselected (selection=0). It should
    // not increment the eliminated count  -  there's nothing to remove.
    let mut sel = vec![1, 0, 1];
    let dead = vec![false, true, false];
    let n = prune_dead_arms_inplace(&mut sel, &dead);
    assert_eq!(sel, vec![1, 0, 1]);
    assert_eq!(n, 0);
}

#[test]
fn prune_dead_arms_returns_zero_on_length_mismatch() {
    let mut sel = vec![1, 1, 1];
    let dead = vec![true, false]; // wrong length
    let n = prune_dead_arms_inplace(&mut sel, &dead);
    assert_eq!(n, 0);
    // Selection must be untouched.
    assert_eq!(sel, vec![1, 1, 1]);
}

#[test]
fn prune_dead_arms_handles_empty_selection() {
    let mut sel: Vec<u32> = vec![];
    let dead: Vec<bool> = vec![];
    let n = prune_dead_arms_inplace(&mut sel, &dead);
    assert_eq!(n, 0);
}

#[test]
fn prune_dead_arms_idempotent_on_repeated_call() {
    let mut sel = vec![1, 1, 0, 1];
    let dead = vec![true, false, true, true];
    let first = prune_dead_arms_inplace(&mut sel, &dead);
    let after_first = sel.clone();
    let second = prune_dead_arms_inplace(&mut sel, &dead);
    assert_eq!(first, 2);
    assert_eq!(second, 0, "second pass must find nothing left to prune");
    assert_eq!(sel, after_first);
}

#[test]
fn prune_dead_arms_preserves_non_zero_unselected_entries() {
    // Defensive: planner cost vectors sometimes carry sentinel
    // values like u32::MAX. The substrate must only zero entries
    // that are dead AND currently selected; it must not stomp on
    // sentinel values it doesn't understand.
    let mut sel = vec![u32::MAX, 1, 1];
    let dead = vec![false, true, false];
    let n = prune_dead_arms_inplace(&mut sel, &dead);
    assert_eq!(sel, vec![u32::MAX, 0, 1]);
    assert_eq!(n, 1);
}

// ── C3: shared prologue extraction tests ─────────────────────────

#[test]
fn shared_prologue_zero_when_arm_list_empty() {
    let arms: [&[MegakernelWorkItem]; 0] = [];
    assert_eq!(shared_prologue_length(&arms), 0);
}

#[test]
fn shared_prologue_zero_when_any_arm_empty() {
    let a = vec![item(1, 0, 0), item(2, 0, 0)];
    let b: Vec<MegakernelWorkItem> = vec![];
    let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
    assert_eq!(shared_prologue_length(&arms), 0);
}

#[test]
fn shared_prologue_zero_when_first_op_differs() {
    let a = vec![item(1, 0, 0), item(2, 0, 0)];
    let b = vec![item(7, 0, 0), item(2, 0, 0)];
    let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
    assert_eq!(shared_prologue_length(&arms), 0);
}

#[test]
fn shared_prologue_returns_full_length_when_all_arms_identical() {
    let a = vec![item(1, 0, 0), item(2, 0, 0), item(3, 0, 0)];
    let arms: [&[MegakernelWorkItem]; 3] = [&a, &a, &a];
    assert_eq!(shared_prologue_length(&arms), 3);
}

#[test]
fn shared_prologue_returns_partial_prefix_when_arms_diverge_midway() {
    // First two ops match, third differs.
    let a = vec![item(1, 0, 0), item(2, 0, 0), item(3, 0, 0)];
    let b = vec![item(1, 0, 0), item(2, 0, 0), item(99, 0, 0)];
    let c = vec![item(1, 0, 0), item(2, 0, 0)];
    let arms: [&[MegakernelWorkItem]; 3] = [&a, &b, &c];
    // c is the shortest at length 2; shared prefix capped at 2.
    assert_eq!(shared_prologue_length(&arms), 2);
}

#[test]
fn shared_prologue_distinguishes_input_handle_difference() {
    // Same op_handle, different input_handle → not equal.
    let a = vec![item(1, 7, 0)];
    let b = vec![item(1, 9, 0)];
    let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
    assert_eq!(shared_prologue_length(&arms), 0);
}

#[test]
fn shared_prologue_capped_by_shortest_arm() {
    let a = vec![item(1, 0, 0), item(2, 0, 0), item(3, 0, 0), item(4, 0, 0)];
    let b = vec![item(1, 0, 0), item(2, 0, 0)];
    let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
    assert_eq!(shared_prologue_length(&arms), 2);
}

#[test]
fn checked_selector_reports_shape_errors() {
    let mut scratch = FusionSelectionScratch::default();
    let err = select_fused_subset_checked_into(&[1.0], 2, &[0, 0, 0, 0], &mut scratch).unwrap_err();
    assert_eq!(
        err,
        FusionSelectionError::CostLen {
            expected: 2,
            actual: 1,
        }
    );

    let err = select_fused_subset_compact_checked_into(&[1, 2], 2, &[0], &mut scratch).unwrap_err();
    assert_eq!(
        err,
        FusionSelectionError::ExchangeAdjLen {
            expected: 4,
            actual: 1,
        }
    );
}

#[test]
#[should_panic(expected = "subset selection failed on malformed planner input")]
fn dense_selector_into_fails_loud_on_shape_error() {
    // Malformed planner input must fail LOUD, not silently clear the scratch
    // (which would degrade to the slower unfused path with no signal; Law 10/7).
    let mut scratch = FusionSelectionScratch::default();
    select_fused_subset_into(&[1.0], 2, &[0, 0, 0, 0], &mut scratch);
}

#[test]
#[should_panic(expected = "subset selection failed on malformed planner input")]
fn compact_selector_into_fails_loud_on_shape_error() {
    let mut scratch = FusionSelectionScratch::default();
    select_fused_subset_compact_into(&[1_u16, 2], 2, &[0], &mut scratch);
}

#[test]
fn compact_plan_order_indexes_items_not_op_handles() {
    let work = [item(500, 1, 2), item(600, 3, 4), item(700, 5, 6)];
    let mut scratch = CompactFusionPlanningScratch::default();

    let selected = plan_compact_fusion_into(&work, &mut scratch).to_vec();

    assert_eq!(selected.len(), 3);
    assert_eq!(selected, vec![1, 1, 1]);
    assert_eq!(scratch.exchange_adj().len(), 9);
}
