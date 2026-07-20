use super::super::state::AdaptiveTraversalPlanCache;
use super::super::*;
use super::support::RecordingResidentDispatcher;

#[test]
fn generated_adaptive_resident_free_releases_each_handle_once_in_first_seen_order() {
    for seed in 0..4096_u64 {
        let dispatcher = RecordingResidentDispatcher::default();
        let base = 20_000 + seed * 16;
        let graph = ResidentAdaptiveTraversalGraph {
            node_count: 4,
            edge_count: 3,
            max_row_degree: 1,
            high_degree_source_count: 0,
            words: 1,
            layout_hash: seed,
            handles: [base, base + 1, base + 2, base],
        };
        graph.free(&dispatcher).expect("Fix: graph free dedup");
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[base, base + 1, base + 2]
        );

        dispatcher.freed.borrow_mut().clear();
        let mut scratch = AdaptiveTraversalResidentScratch {
            handles: Some([base + 3, base + 4, base + 3]),
            queue_handle: Some(base + 4),
            high_queue_handle: Some(base + 6),
            high_len_handle: Some(base + 6),
            word_partials_handle: Some(base + 5),
            word_block_totals_handle: Some(base + 5),
            frontier_bytes: 4,
            queue_bytes: 4,
            high_queue_bytes: 4,
            word_partials_bytes: 4,
            word_block_totals_bytes: 4,
            frontier_in_bytes: Vec::new(),
            readbacks: Vec::new(),
            plan_cache: AdaptiveTraversalPlanCache::default(),
        };
        scratch.free(&dispatcher).expect("Fix: scratch free dedup");
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[base + 3, base + 4, base + 6, base + 5]
        );
    }
}
