use super::super::*;
use super::support::RecordingResidentDispatcher;
use crate::graph::csr_frontier_queue_scratch::ResidentCsrQueueMaterializer;

#[test]
fn generated_resident_csr_queue_free_releases_each_handle_once_in_first_seen_order() {
    for seed in 0..4096_u64 {
        let dispatcher = RecordingResidentDispatcher::default();
        let base = 30_000 + seed * 16;
        let graph = ResidentCsrQueueGraph {
            node_count: 4,
            edge_count: 3,
            max_row_degree: 1,
            high_degree_source_count: 0,
            words: 1,
            edge_offsets_handle: base,
            edge_targets_handle: base + 1,
            edge_kind_mask_handle: base,
        };
        graph.free(&dispatcher).expect("Fix: graph free dedup");
        assert_eq!(dispatcher.freed.borrow().as_slice(), &[base, base + 1]);

        dispatcher.freed.borrow_mut().clear();
        let mut scratch = ResidentCsrQueueScratch::default();
        scratch.handles = Some(ResidentCsrQueueScratchHandles {
            frontier: base + 2,
            active_queue: base + 2,
            queue_len: base + 3,
            frontier_out: base + 4,
            word_partials: None,
            block_totals: None,
            high_queue: None,
            high_len: None,
            queue_capacity: 4,
            high_queue_capacity: 0,
            frontier_bytes: 4,
            materializer: ResidentCsrQueueMaterializer::AtomicWordScan,
        });
        scratch.free(&dispatcher).expect("Fix: scratch free dedup");
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[base + 2, base + 3, base + 4]
        );

        dispatcher.freed.borrow_mut().clear();
        scratch.handles = Some(ResidentCsrQueueScratchHandles {
            frontier: base + 5,
            active_queue: base + 6,
            queue_len: base + 6,
            frontier_out: base + 7,
            word_partials: Some(base + 8),
            block_totals: Some(base + 8),
            high_queue: Some(base + 9),
            high_len: Some(base + 9),
            queue_capacity: 4,
            high_queue_capacity: 1,
            frontier_bytes: 4,
            materializer: ResidentCsrQueueMaterializer::DeterministicWordPrefix,
        });
        scratch
            .free(&dispatcher)
            .expect("Fix: word-prefix scratch free dedup");
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[base + 5, base + 6, base + 7, base + 8, base + 9]
        );
    }
}
