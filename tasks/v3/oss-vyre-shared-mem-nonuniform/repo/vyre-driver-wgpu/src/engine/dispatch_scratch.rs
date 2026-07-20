//! Per-thread scratch arenas for record/readback hot-path POD vectors.
//!
//! Audit P0 #9: dispatch-local `SmallVec` spills go through reusable
//! thread-local `Vec<T>` capacity instead of heap-allocating fresh on every
//! dispatch when the program exceeds the inline cap. Small programs still pay
//! zero allocations because the scratch grows monotonically and is cleared
//! (not freed) between dispatches.
//!
//! Only POD/`Copy` element types live here; vectors that carry owned
//! `PooledBuffer`/`Arc<BindGroup>`/borrowed references stay local to
//! `record_and_submit_async` because their drop or lifetime semantics cannot
//! be hoisted across calls.
//!
//! The scratch is taken by mutable borrow for the duration of one dispatch via
//! `with_dispatch_scratch`. `record_and_submit_async` is non-reentrant on a
//! single thread; a nested borrow is a backend invariant violation.

use std::cell::RefCell;

use super::record_and_readback::binding_lookup::BindingLookup;
use vyre_driver::BackendError;

/// Reusable per-thread vectors borrowed for one dispatch.
pub(crate) struct DispatchScratch {
    /// `(binding, offset, len)` tuples for buffer regions that must be
    /// zero-cleared after upload but before dispatch.
    pub(crate) clear_requests: Vec<(u32, u64, u64)>,
    /// Per-bind-group buffer ids in declaration order, used as the cache key
    /// for `BindGroupCache::get_by_ids`.
    pub(crate) bind_group_buffer_ids: Vec<u64>,
    /// Per-bind-group `gpu_buffers` indices resolved before constructing
    /// `wgpu::BindGroupEntry` slices. Each index resolves to
    /// `(binding, &PooledBuffer)` via the local `gpu_buffers` array. Storing
    /// indices instead of references lets the scratch live in a thread-local.
    pub(crate) bind_group_bound_indices: Vec<usize>,
    /// Binding-to-input index lookup reused across dispatches.
    pub(crate) input_idx_by_binding: BindingLookup,
    /// Binding-to-GPU-buffer index lookup reused across dispatches.
    pub(crate) gpu_idx_by_binding: BindingLookup,
    /// Binding-to-output-layout index lookup reused across dispatches.
    pub(crate) output_idx_by_binding: BindingLookup,
}

impl DispatchScratch {
    fn new() -> Self {
        Self {
            clear_requests: Vec::new(),
            bind_group_buffer_ids: Vec::new(),
            bind_group_bound_indices: Vec::new(),
            input_idx_by_binding: BindingLookup::new(),
            gpu_idx_by_binding: BindingLookup::new(),
            output_idx_by_binding: BindingLookup::new(),
        }
    }

    /// Reset every vector for the next dispatch without releasing capacity.
    fn reset(&mut self) {
        self.clear_requests.clear();
        self.bind_group_buffer_ids.clear();
        self.bind_group_bound_indices.clear();
        self.input_idx_by_binding.clear();
        self.gpu_idx_by_binding.clear();
        self.output_idx_by_binding.clear();
    }
}

thread_local! {
    static SCRATCH: RefCell<DispatchScratch> = RefCell::new(DispatchScratch::new());
}

/// Run `f` with exclusive borrow of the per-thread dispatch scratch.
///
/// The scratch is `reset()` on entry so callers see empty vectors with
/// retained capacity. Nested dispatch on the same thread is rejected loudly:
/// allocating a second arena here would hide a hot-path ownership bug.
pub(crate) fn with_dispatch_scratch<F, R>(f: F) -> Result<R, BackendError>
where
    F: FnOnce(&mut DispatchScratch) -> Result<R, BackendError>,
{
    SCRATCH.with(|cell| {
        let mut scratch = cell.try_borrow_mut().map_err(|_| {
            BackendError::new(
                "re-entrant wgpu dispatch scratch borrow. Fix: do not dispatch recursively on the same worker thread; submit nested work after the outer dispatch returns.",
            )
        })?;
        scratch.reset();
        f(&mut scratch)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_retains_capacity_across_dispatches() {
        with_dispatch_scratch(|scratch| {
            for binding in 0..32u32 {
                scratch.clear_requests.push((binding, 0, 4));
                scratch.bind_group_buffer_ids.push(u64::from(binding));
                scratch.bind_group_bound_indices.push(binding as usize);
            }
            Ok(())
        })
        .expect("Fix: dispatch scratch first borrow should succeed");

        with_dispatch_scratch(|scratch| {
            assert!(scratch.clear_requests.is_empty());
            assert!(scratch.bind_group_buffer_ids.is_empty());
            assert!(scratch.bind_group_bound_indices.is_empty());
            assert!(
                scratch.clear_requests.capacity() >= 32,
                "Fix: dispatch scratch must retain capacity across calls. \
                 Got {} clear_requests capacity.",
                scratch.clear_requests.capacity()
            );
            assert!(
                scratch.bind_group_buffer_ids.capacity() >= 32,
                "Fix: dispatch scratch must retain capacity across calls. \
                 Got {} bind_group_buffer_ids capacity.",
                scratch.bind_group_buffer_ids.capacity()
            );
            assert!(
                scratch.bind_group_bound_indices.capacity() >= 32,
                "Fix: dispatch scratch must retain capacity across calls. \
                 Got {} bind_group_bound_indices capacity.",
                scratch.bind_group_bound_indices.capacity()
            );
            Ok(())
        })
        .expect("Fix: dispatch scratch second borrow should succeed");
    }

    #[test]
    fn nested_call_returns_structured_error() {
        let error = with_dispatch_scratch(|outer| {
            outer.clear_requests.push((1, 0, 4));
            with_dispatch_scratch(|inner| {
                assert!(inner.clear_requests.is_empty());
                inner.clear_requests.push((2, 0, 8));
                Ok(())
            })
        })
        .expect_err("nested dispatch scratch borrow must return an error");
        assert!(
            error
                .to_string()
                .contains("re-entrant wgpu dispatch scratch borrow"),
            "unexpected error: {error}"
        );
    }
}
