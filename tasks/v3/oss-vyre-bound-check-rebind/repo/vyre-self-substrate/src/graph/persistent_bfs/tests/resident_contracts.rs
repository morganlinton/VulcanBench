use super::super::state::PersistentBfsPlanCache;
use super::super::*;
use super::linear_graph;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher, ResidentReadRange};
use std::cell::{Cell, RefCell};
use vyre_foundation::ir::Program;

#[derive(Default)]
struct ResidentPersistentBfsDispatcher {
    next_handle: RefCell<u64>,
    device_features: Cell<u64>,
    alloc_attempts: Cell<usize>,
    fail_alloc_attempt: Cell<Option<usize>>,
    alloc_sizes: RefCell<Vec<usize>>,
    topology_upload_batch_sizes: RefCell<Vec<usize>>,
    query_upload_batch_sizes: RefCell<Vec<usize>>,
    step_handle_sets: RefCell<Vec<Vec<u64>>>,
    freed: RefCell<Vec<u64>>,
}

impl ResidentPersistentBfsDispatcher {
    fn new() -> Self {
        Self {
            next_handle: RefCell::new(10),
            ..Self::default()
        }
    }
}

impl OptimizerDispatcher for ResidentPersistentBfsDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: resident persistent BFS test dispatcher only supports resident APIs.".to_string(),
        ))
    }

    fn supports_persistent(&self) -> bool {
        true
    }

    fn device_feature_cache_key(&self) -> u64 {
        self.device_features.get()
    }

    fn alloc_resident(&self, byte_len: usize) -> Result<u64, DispatchError> {
        let attempt = self.alloc_attempts.get() + 1;
        self.alloc_attempts.set(attempt);
        if self.fail_alloc_attempt.get() == Some(attempt) {
            return Err(DispatchError::BackendError(format!(
                "Fix: injected resident allocation failure at attempt {attempt}."
            )));
        }
        let mut next = self.next_handle.borrow_mut();
        let handle = *next;
        *next += 1;
        self.alloc_sizes.borrow_mut().push(byte_len);
        Ok(handle)
    }

    fn upload_resident_many(&self, uploads: &[(u64, &[u8])]) -> Result<(), DispatchError> {
        self.topology_upload_batch_sizes
            .borrow_mut()
            .push(uploads.len());
        Ok(())
    }

    fn upload_resident_many_sequence_read_ranges_into(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[crate::optimizer::dispatcher::ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        assert_eq!(uploads.len(), 1);
        assert_eq!(steps.len(), 1);
        assert_eq!(read_ranges.len(), 2);
        self.query_upload_batch_sizes
            .borrow_mut()
            .push(uploads.len());
        self.step_handle_sets
            .borrow_mut()
            .push(steps[0].handle_ids.to_vec());
        outputs.clear();
        let frontier_words = read_ranges[0].byte_len / std::mem::size_of::<u32>();
        let changed_words = read_ranges[1].byte_len / std::mem::size_of::<u32>();
        outputs.push(u32_slice_to_le_bytes(&vec![0b1111; frontier_words]));
        outputs.push(u32_slice_to_le_bytes(&vec![1; changed_words]));
        Ok(())
    }

    fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
        self.freed.borrow_mut().push(handle);
        Ok(())
    }
}

#[test]
fn resident_graph_uploads_topology_once_and_reuses_frontier_handles() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    let (off, tgt, msk) = linear_graph();
    let graph =
        upload_resident_bfs_graph(&dispatcher, 4, &off, &tgt, &msk).expect("Fix: resident upload");
    assert_eq!(
        dispatcher.topology_upload_batch_sizes.borrow().as_slice(),
        &[4]
    );
    assert_eq!(dispatcher.alloc_sizes.borrow().len(), 4);

    let graph_handles = graph.handles();
    assert_eq!(
        graph_handles[0], graph_handles[4],
        "resident BFS must bind one uploaded zero node buffer to both ProgramGraph node slots"
    );
    let mut scratch = PersistentBfsResidentScratch::default();
    let mut frontier = Vec::with_capacity(4);
    let frontier_ptr = frontier.as_ptr();
    let changed = bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: first resident query");
    assert_eq!(changed, 1);
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(frontier.as_ptr(), frontier_ptr);
    assert_eq!(dispatcher.alloc_sizes.borrow().len(), 7);

    let changed = bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0011],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: second resident query");
    assert_eq!(changed, 1);
    assert_eq!(dispatcher.alloc_sizes.borrow().len(), 7);
    assert_eq!(
        dispatcher.query_upload_batch_sizes.borrow().as_slice(),
        &[1, 1]
    );
    assert_eq!(
        scratch.plan_cache_snapshot(),
        PersistentBfsPlanCacheSnapshot {
            entries: 1,
            hits: 1,
            misses: 1,
        }
    );

    let step_handles = dispatcher.step_handle_sets.borrow();
    assert_eq!(step_handles.len(), 2);
    assert_eq!(&step_handles[0][0..5], &graph_handles);
    assert_eq!(&step_handles[1][0..5], &graph_handles);
    assert_eq!(
        &step_handles[0][5..8],
        &step_handles[1][5..8],
        "frontier/change resident buffers must be reused across queries"
    );

    scratch.free(&dispatcher).expect("Fix: scratch free");
    graph.free(&dispatcher).expect("Fix: graph free");
    assert_eq!(dispatcher.freed.borrow().len(), 7);
}

#[test]
fn resident_single_zero_iters_returns_seed_without_query_allocation_or_dispatch() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    let (off, tgt, msk) = linear_graph();
    let graph =
        upload_resident_bfs_graph(&dispatcher, 4, &off, &tgt, &msk).expect("Fix: resident upload");
    let topology_allocs = dispatcher.alloc_sizes.borrow().len();
    let mut scratch = PersistentBfsResidentScratch::default();
    let mut frontier = Vec::with_capacity(4);
    let ptr = frontier.as_ptr();

    let changed = bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0101],
        0xFFFF_FFFF,
        0,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: zero-iteration resident BFS should validate and return seed frontier");

    assert_eq!(changed, 0);
    assert_eq!(frontier, vec![0b0101]);
    assert_eq!(frontier.as_ptr(), ptr);
    assert_eq!(dispatcher.alloc_sizes.borrow().len(), topology_allocs);
    assert!(dispatcher.query_upload_batch_sizes.borrow().is_empty());
    assert!(dispatcher.step_handle_sets.borrow().is_empty());
    assert_eq!(
        scratch.plan_cache_snapshot(),
        PersistentBfsPlanCacheSnapshot::default()
    );

    graph.free(&dispatcher).expect("Fix: graph free");
}

#[test]
fn generated_resident_bfs_free_releases_each_handle_once_in_first_seen_order() {
    for seed in 0..4096_u64 {
        let dispatcher = ResidentPersistentBfsDispatcher::new();
        let base = 10_000 + seed * 16;
        let graph = ResidentBfsGraph {
            node_count: 4,
            edge_count: 3,
            words: 1,
            words_u32: 1,
            layout_hash: seed,
            handles: [base, base + 1, base + 2, base + 3, base],
        };
        graph.free(&dispatcher).expect("Fix: graph free dedup");
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[base, base + 1, base + 2, base + 3]
        );

        dispatcher.freed.borrow_mut().clear();
        let mut scratch = PersistentBfsResidentScratch {
            frontier_handles: Some([base + 4, base + 4, base + 5]),
            frontier_bytes: 4,
            changed_bytes: 4,
            frontier_in_bytes: Vec::new(),
            readbacks: Vec::new(),
            changed: Vec::new(),
            plan_cache: PersistentBfsPlanCache::default(),
        };
        scratch.free(&dispatcher).expect("Fix: scratch free dedup");
        assert_eq!(dispatcher.freed.borrow().as_slice(), &[base + 4, base + 5]);
    }
}

#[test]
fn resident_query_handle_allocation_rolls_back_partial_allocations() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    dispatcher.fail_alloc_attempt.set(Some(3));
    let mut scratch = PersistentBfsResidentScratch::default();

    let err = ensure_resident_query_handles(&dispatcher, &mut scratch, 64, 4)
        .expect_err("third resident scratch allocation failure must fail the whole acquisition");

    assert!(
        err.to_string()
            .contains("injected resident allocation failure at attempt 3"),
        "Fix: scratch allocation rollback must preserve the original allocation failure, got: {err}"
    );
    assert_eq!(
        dispatcher.freed.borrow().as_slice(),
        &[10, 11],
        "Fix: failed multi-handle resident BFS scratch acquisition must free every earlier handle."
    );
    assert!(
        scratch.frontier_handles.is_none(),
        "Fix: failed scratch acquisition must not publish partial resident handles."
    );
    assert_eq!(scratch.frontier_bytes, 0);
    assert_eq!(scratch.changed_bytes, 0);
}

#[test]
fn resident_graph_batch_reuses_topology_and_frontier_handles() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    let (off, tgt, msk) = linear_graph();
    let graph =
        upload_resident_bfs_graph(&dispatcher, 4, &off, &tgt, &msk).expect("Fix: resident upload");
    assert_eq!(graph.words(), 1);

    let mut scratch = PersistentBfsResidentScratch::default();
    let mut frontiers = Vec::with_capacity(4);
    let frontiers_ptr = frontiers.as_ptr();
    let mut changed = Vec::with_capacity(4);
    let changed_ptr = changed.as_ptr();
    bfs_expand_resident_graph_batch_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0001, 0b0011, 0b0111],
        3,
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontiers,
        &mut changed,
    )
    .expect("Fix: resident batch query");

    assert_eq!(frontiers, vec![0b1111, 0b1111, 0b1111]);
    assert_eq!(changed, vec![1, 1, 1]);
    assert_eq!(frontiers.as_ptr(), frontiers_ptr);
    assert_eq!(changed.as_ptr(), changed_ptr);
    assert_eq!(
        dispatcher.topology_upload_batch_sizes.borrow().as_slice(),
        &[4]
    );
    assert_eq!(
        dispatcher.query_upload_batch_sizes.borrow().as_slice(),
        &[1]
    );
    assert_eq!(dispatcher.alloc_sizes.borrow().len(), 7);
    assert_eq!(
        scratch.plan_cache_snapshot(),
        PersistentBfsPlanCacheSnapshot {
            entries: 1,
            hits: 0,
            misses: 1,
        }
    );

    let step_handles = dispatcher.step_handle_sets.borrow();
    assert_eq!(step_handles.len(), 1);

    scratch.free(&dispatcher).expect("Fix: scratch free");
    graph.free(&dispatcher).expect("Fix: graph free");
}

#[test]
fn resident_batch_zero_iters_returns_seed_and_zero_changed_without_query_allocation_or_dispatch() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    let (off, tgt, msk) = linear_graph();
    let graph =
        upload_resident_bfs_graph(&dispatcher, 4, &off, &tgt, &msk).expect("Fix: resident upload");
    let topology_allocs = dispatcher.alloc_sizes.borrow().len();
    let mut scratch = PersistentBfsResidentScratch::default();
    let mut frontiers = Vec::with_capacity(4);
    let frontiers_ptr = frontiers.as_ptr();
    let mut changed = Vec::with_capacity(4);
    let changed_ptr = changed.as_ptr();

    bfs_expand_resident_graph_batch_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0001, 0b0011, 0b0101],
        3,
        0xFFFF_FFFF,
        0,
        &mut scratch,
        &mut frontiers,
        &mut changed,
    )
    .expect("Fix: zero-iteration resident batch should validate and return seed frontiers");

    assert_eq!(frontiers, vec![0b0001, 0b0011, 0b0101]);
    assert_eq!(changed, vec![0, 0, 0]);
    assert_eq!(frontiers.as_ptr(), frontiers_ptr);
    assert_eq!(changed.as_ptr(), changed_ptr);
    assert_eq!(dispatcher.alloc_sizes.borrow().len(), topology_allocs);
    assert!(dispatcher.query_upload_batch_sizes.borrow().is_empty());
    assert!(dispatcher.step_handle_sets.borrow().is_empty());
    assert_eq!(
        scratch.plan_cache_snapshot(),
        PersistentBfsPlanCacheSnapshot::default()
    );

    graph.free(&dispatcher).expect("Fix: graph free");
}

#[test]
fn resident_plan_cache_keys_include_device_features() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    let (off, tgt, msk) = linear_graph();
    let graph =
        upload_resident_bfs_graph(&dispatcher, 4, &off, &tgt, &msk).expect("Fix: resident upload");
    let mut scratch = PersistentBfsResidentScratch::default();
    let mut frontier = Vec::new();

    dispatcher.device_features.set(0x10);
    bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: first feature-keyed query");
    dispatcher.device_features.set(0x20);
    bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: second feature-keyed query");

    assert_eq!(
        scratch.plan_cache_snapshot(),
        PersistentBfsPlanCacheSnapshot {
            entries: 2,
            hits: 0,
            misses: 2,
        },
        "plan cache key must include backend device/lowering features"
    );

    scratch.free(&dispatcher).expect("Fix: scratch free");
    graph.free(&dispatcher).expect("Fix: graph free");
}

#[test]
fn resident_plan_cache_reuses_same_shape_graph_programs() {
    let dispatcher = ResidentPersistentBfsDispatcher::new();
    let offsets = vec![0, 1, 2, 3, 3];
    let masks = vec![1, 1, 1];
    let graph_a = upload_resident_bfs_graph(&dispatcher, 4, &offsets, &[1, 2, 3], &masks)
        .expect("Fix: first resident graph upload");
    let graph_b = upload_resident_bfs_graph(&dispatcher, 4, &offsets, &[2, 3, 0], &masks)
        .expect("Fix: second resident graph upload");
    assert_ne!(graph_a.layout_hash(), graph_b.layout_hash());

    let mut scratch = PersistentBfsResidentScratch::default();
    let mut frontier = Vec::new();
    bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph_a,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: first resident query");
    bfs_expand_resident_graph_with_scratch_into(
        &dispatcher,
        &graph_b,
        &[0b0001],
        0xFFFF_FFFF,
        4,
        &mut scratch,
        &mut frontier,
    )
    .expect("Fix: second resident query");

    assert_eq!(
        scratch.plan_cache_snapshot(),
        PersistentBfsPlanCacheSnapshot {
            entries: 1,
            hits: 1,
            misses: 1,
        },
        "resident BFS programs must be cached by program shape, not graph contents"
    );

    scratch.free(&dispatcher).expect("Fix: scratch free");
    graph_a.free(&dispatcher).expect("Fix: first graph free");
    graph_b.free(&dispatcher).expect("Fix: second graph free");
}
