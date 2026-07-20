//! CUDA-resident `OptimizerDispatcher`  -  the fast path for the
//! self-hosted optimizer.
//!
//! Implements the persistent surface of `OptimizerDispatcher`: alloc
//! once, upload once, dispatch many times against the same resident
//! buffers, read back at the end. This bypasses the per-call sync
//! overhead the borrowed `dispatch` API has, which is the dominant
//! cost on the optimizer's multi-pass pipeline at small input sizes.
//!
//! CUDA is the persistent optimizer release path. Non-CUDA dispatchers must
//! select their explicit borrowed-dispatch route through capability probing;
//! they must not masquerade as resident execution or silently degrade a CUDA
//! residency contract.

use std::cell::RefCell;

use rustc_hash::FxHashMap;
use vyre_driver::accounting::checked_add_u64_lazy;
use vyre_driver::input_identity::{domain_separated_exact_input_key, ExactInputKey};
use vyre_driver::DispatchConfig;
use vyre_foundation::ir::Program;
use vyre_self_substrate::optimizer::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
    ResidentStaticBufferSet,
};

use crate::backend::output_range::CudaOutputReadback;
use crate::backend::staging_reserve::reserve_vec;
use crate::backend::{CudaBackend, CudaResidentBuffer, CudaResidentDispatchStep};
use crate::numeric::CUDA_NUMERIC;

const CUDA_OPTIMIZER_RESIDENT_POOL_BUDGET_DENOMINATOR: u64 = 32;

struct StaticUploadCacheEntry {
    handles: Vec<CudaResidentBuffer>,
    bytes: u64,
}

fn reserve_optimizer_vec<T>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), DispatchError> {
    reserve_vec(vec, capacity, field)
        .map_err(|error| DispatchError::BackendError(error.to_string()))
}

/// Optimizer dispatcher backed by CUDA-resident buffers.
///
/// Holds a borrow on a live [`CudaBackend`]. All `OptimizerDispatcher`
/// trait methods route through the backend's resident-buffer surface
/// when the persistent path applies; the borrowed `dispatch` method
/// still exists for transitions between passes that haven't been
/// converted to persistent yet.
///
/// **Persistent buffer pool.** `free_resident` does NOT actually free
/// the underlying CUDA allocation  -  it returns the handle to a
/// per-byte-len free list owned by this dispatcher. Subsequent
/// `alloc_resident` calls with the same `byte_len` reuse a pooled
/// handle in O(1), bypassing the ~3-5ms CUDA `cuMemAlloc`
/// round-trip. Real alloc/free fires only on size-class misses or
/// when the dispatcher is dropped (see `Drop` impl). For a multi-
/// pass optimizer that allocates 14+ buffers per pipeline run, this
/// drops alloc cost from ~50ms/run on the first call to ~µs on
/// every subsequent call.
pub struct CudaOptimizerDispatcher<'a> {
    backend: &'a CudaBackend,
    /// `id → byte_len` for resident handles we've allocated. The
    /// CUDA-side `CudaResidentBuffer` handle bundles `id + byte_len`;
    /// when a caller hands us back just an `id`, we look up the
    /// `byte_len` here to reconstruct it.
    sizes: RefCell<FxHashMap<u64, usize>>,
    /// Per-`byte_len` free list. `free_resident` pushes onto the list
    /// instead of calling the backend; `alloc_resident` pops first
    /// before falling back to a real allocation.
    free_pool: RefCell<FxHashMap<usize, Vec<CudaResidentBuffer>>>,
    /// Bytes currently retained by `free_pool`.
    pooled_bytes: RefCell<u64>,
    /// Content-addressed immutable resident payloads. These handles stay live
    /// across optimizer calls so warmed CUDA runs skip repeated H2D upload of
    /// graph/arena buffers.
    static_upload_cache: RefCell<FxHashMap<ExactInputKey, StaticUploadCacheEntry>>,
    /// Bytes currently retained by `static_upload_cache`.
    static_cached_bytes: RefCell<u64>,
    /// Hard cap for idle resident handles retained by this dispatcher.
    max_pooled_bytes: u64,
    /// Hard cap for immutable resident payload handles retained by this
    /// dispatcher.
    max_static_cached_bytes: u64,
}

impl<'a> CudaOptimizerDispatcher<'a> {
    /// Wrap a live `CudaBackend` for use as an `OptimizerDispatcher`.
    pub fn new(backend: &'a CudaBackend) -> Self {
        Self::with_pool_budget(
            backend,
            cuda_optimizer_resident_pool_budget_bytes(backend.device_memory_bytes()),
        )
    }

    fn with_pool_budget(backend: &'a CudaBackend, max_pooled_bytes: u64) -> Self {
        Self {
            backend,
            sizes: RefCell::new(FxHashMap::default()),
            free_pool: RefCell::new(FxHashMap::default()),
            pooled_bytes: RefCell::new(0),
            static_upload_cache: RefCell::new(FxHashMap::default()),
            static_cached_bytes: RefCell::new(0),
            max_pooled_bytes,
            max_static_cached_bytes: max_pooled_bytes,
        }
    }

    #[cfg(test)]
    fn new_with_pool_budget_for_tests(backend: &'a CudaBackend, max_pooled_bytes: u64) -> Self {
        Self::with_pool_budget(backend, max_pooled_bytes)
    }

    fn resolve(&self, id: u64) -> Result<CudaResidentBuffer, DispatchError> {
        let sizes = self.sizes.borrow();
        let byte_len = sizes.get(&id).copied().ok_or_else(|| {
            DispatchError::Rejected(format!(
                "Fix: CUDA optimizer dispatcher received unknown resident handle id {id}; \
                 every id must come from this dispatcher's `alloc_resident`."
            ))
        })?;
        Ok(CudaResidentBuffer { id, byte_len })
    }

    fn resolve_many(&self, ids: &[u64]) -> Result<Vec<CudaResidentBuffer>, DispatchError> {
        let mut handles = Vec::new();
        reserve_optimizer_vec(&mut handles, ids.len(), "optimizer resident handle")?;
        for &id in ids {
            handles.push(self.resolve(id)?);
        }
        Ok(handles)
    }

    fn resolve_uploads<'b>(
        &self,
        uploads: &[(u64, &'b [u8])],
    ) -> Result<Vec<(CudaResidentBuffer, &'b [u8])>, DispatchError> {
        let mut concrete = Vec::new();
        reserve_optimizer_vec(&mut concrete, uploads.len(), "optimizer upload")?;
        for &(id, bytes) in uploads {
            concrete.push((self.resolve(id)?, bytes));
        }
        Ok(concrete)
    }

    fn resolve_read_ranges(
        &self,
        ranges: &[ResidentReadRange],
    ) -> Result<(Vec<CudaResidentBuffer>, Vec<CudaOutputReadback>), DispatchError> {
        let mut handles = Vec::new();
        reserve_optimizer_vec(&mut handles, ranges.len(), "optimizer readback handle")?;
        let mut readbacks = Vec::new();
        reserve_optimizer_vec(&mut readbacks, ranges.len(), "optimizer readback range")?;
        for range in ranges {
            handles.push(self.resolve(range.handle_id)?);
            readbacks.push(CudaOutputReadback {
                device_offset: range.byte_offset,
                byte_len: range.byte_len,
            });
        }
        Ok((handles, readbacks))
    }

    fn resolve_clears(
        &self,
        clears: &[(u64, usize)],
    ) -> Result<Vec<CudaResidentBuffer>, DispatchError> {
        let mut handles = Vec::new();
        reserve_optimizer_vec(&mut handles, clears.len(), "optimizer clear handle")?;
        for &(id, byte_len) in clears {
            let handle = self.resolve(id)?;
            if handle.byte_len != byte_len {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: CUDA optimizer resident clear for handle {id} expected full buffer length {} but caller requested {byte_len}.",
                    handle.byte_len
                )));
            }
            handles.push(handle);
        }
        Ok(handles)
    }

    fn resolve_fills(
        &self,
        fills: &[(u64, usize, u8)],
    ) -> Result<Vec<(CudaResidentBuffer, u8)>, DispatchError> {
        let mut resolved = Vec::new();
        reserve_optimizer_vec(&mut resolved, fills.len(), "optimizer fill handle")?;
        for &(id, byte_len, value) in fills {
            let handle = self.resolve(id)?;
            if handle.byte_len != byte_len {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: CUDA optimizer resident fill for handle {id} expected full buffer length {} but caller requested {byte_len}.",
                    handle.byte_len
                )));
            }
            resolved.push((handle, value));
        }
        Ok(resolved)
    }

    fn resolve_step_handles(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        field: &'static str,
    ) -> Result<Vec<Vec<CudaResidentBuffer>>, DispatchError> {
        let mut resolved_step_handles = Vec::new();
        reserve_optimizer_vec(&mut resolved_step_handles, steps.len(), field)?;
        for step in steps {
            resolved_step_handles.push(self.resolve_many(step.handle_ids)?);
        }
        Ok(resolved_step_handles)
    }

    fn build_cuda_steps<'step, 'program>(
        &self,
        steps: &'step [ResidentDispatchStep<'program>],
        resolved_step_handles: &'step [Vec<CudaResidentBuffer>],
        field: &'static str,
    ) -> Result<Vec<CudaResidentDispatchStep<'step>>, DispatchError>
    where
        'program: 'step,
    {
        let mut cuda_steps = Vec::new();
        reserve_optimizer_vec(&mut cuda_steps, steps.len(), field)?;
        for (step, handles) in steps.iter().zip(resolved_step_handles.iter()) {
            let mut config = DispatchConfig::default();
            config.grid_override = step.grid_override;
            cuda_steps.push(CudaResidentDispatchStep {
                program: step.program,
                handles,
                config,
            });
        }
        Ok(cuda_steps)
    }

    /// Drain the per-size free pool and return all pooled handles to
    /// the backend. Called from `Drop` so the CUDA context isn't
    /// leaking allocations after the dispatcher is gone.
    fn drain_pool(&self) {
        let mut pool = self.free_pool.borrow_mut();
        let mut sizes = self.sizes.borrow_mut();
        for (_byte_len, handles) in pool.drain() {
            for handle in handles {
                sizes.remove(&handle.id);
                let _ = self.backend.free_resident(handle);
            }
        }
        *self.pooled_bytes.borrow_mut() = 0;
    }

    fn drain_static_upload_cache(&self) {
        let mut cache = self.static_upload_cache.borrow_mut();
        let mut sizes = self.sizes.borrow_mut();
        for (_key, entry) in cache.drain() {
            for handle in entry.handles {
                sizes.remove(&handle.id);
                let _ = self.backend.free_resident(handle);
            }
        }
        *self.static_cached_bytes.borrow_mut() = 0;
    }

    fn evict_one_pooled_resident(&self) -> Result<bool, DispatchError> {
        let mut pool = self.free_pool.borrow_mut();
        let Some(byte_len) = pool
            .iter()
            .filter(|(_, handles)| !handles.is_empty())
            .map(|(byte_len, _)| *byte_len)
            .max()
        else {
            return Ok(false);
        };
        let Some(handles) = pool.get_mut(&byte_len) else {
            return Ok(false);
        };
        let Some(handle) = handles.pop() else {
            return Ok(false);
        };
        drop(pool);
        {
            let mut pooled_bytes = self.pooled_bytes.borrow_mut();
            let handle_bytes =
                optimizer_usize_to_u64(handle.byte_len, "resident pool evicted handle bytes")?;
            *pooled_bytes = pooled_bytes.checked_sub(handle_bytes).ok_or_else(|| {
                DispatchError::BackendError(
                    "CUDA optimizer resident pool byte accounting underflowed during eviction"
                        .to_string(),
                )
            })?;
        }
        self.backend
            .free_resident(handle)
            .map_err(|e| DispatchError::BackendError(e.to_string()))?;
        Ok(true)
    }

    fn evict_until_resident_pool_has_room(
        &self,
        incoming_bytes: u64,
    ) -> Result<bool, DispatchError> {
        if incoming_bytes > self.max_pooled_bytes {
            return Ok(false);
        }
        while *self.pooled_bytes.borrow() > self.max_pooled_bytes - incoming_bytes {
            if !self.evict_one_pooled_resident()? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn static_upload_cache_key(
        &self,
        cache_domain: u64,
        payloads: &[&[u8]],
    ) -> Result<ExactInputKey, DispatchError> {
        domain_separated_exact_input_key(
            b"vyre.cuda.optimizer.static-upload.v1",
            cache_domain,
            self.device_feature_cache_key(),
            payloads,
        )
        .map_err(|error| DispatchError::BackendError(error.to_string()))
    }

    fn static_payload_bytes(&self, payloads: &[&[u8]]) -> Result<u64, DispatchError> {
        let mut bytes = 0_u64;
        for payload in payloads {
            let payload_bytes = optimizer_usize_to_u64(payload.len(), "static payload byte total")?;
            bytes = checked_add_u64_lazy(bytes, payload_bytes, || {
                DispatchError::BackendError(
                    "CUDA optimizer static payload byte accounting overflowed".to_string(),
                )
            })?;
        }
        Ok(bytes)
    }

    fn evict_one_static_upload_cache_entry(&self) -> Result<bool, DispatchError> {
        let Some(key) = self
            .static_upload_cache
            .borrow()
            .iter()
            .max_by_key(|(_, entry)| entry.bytes)
            .map(|(key, _)| *key)
        else {
            return Ok(false);
        };
        let Some(entry) = self.static_upload_cache.borrow_mut().remove(&key) else {
            return Ok(false);
        };
        {
            let mut cached_bytes = self.static_cached_bytes.borrow_mut();
            *cached_bytes = cached_bytes.checked_sub(entry.bytes).ok_or_else(|| {
                DispatchError::BackendError(
                    "CUDA optimizer static cache byte accounting underflowed during eviction"
                        .to_string(),
                )
            })?;
        }
        let mut sizes = self.sizes.borrow_mut();
        for handle in entry.handles {
            sizes.remove(&handle.id);
            self.backend
                .free_resident(handle)
                .map_err(|e| DispatchError::BackendError(e.to_string()))?;
        }
        Ok(true)
    }

    fn evict_until_static_upload_cache_has_room(
        &self,
        incoming_bytes: u64,
    ) -> Result<bool, DispatchError> {
        if incoming_bytes > self.max_static_cached_bytes {
            return Ok(false);
        }
        while *self.static_cached_bytes.borrow() > self.max_static_cached_bytes - incoming_bytes {
            if !self.evict_one_static_upload_cache_entry()? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher;

    use super::CudaOptimizerDispatcher;
    use crate::backend::CudaBackend;

    #[test]
    fn cuda_optimizer_resident_pool_enforces_byte_budget() {
        let backend = CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
        let baseline = backend.resident_allocated_bytes();

        {
            let dispatcher = CudaOptimizerDispatcher::new_with_pool_budget_for_tests(&backend, 64);
            let first = dispatcher
                .alloc_resident(64)
                .expect("Fix: first resident optimizer allocation must succeed.");
            let second = dispatcher
                .alloc_resident(64)
                .expect("Fix: second resident optimizer allocation must succeed.");
            assert_eq!(
                backend.resident_allocated_bytes(),
                baseline + 128,
                "Fix: live CUDA resident accounting must include both active optimizer buffers."
            );

            dispatcher
                .free_resident(first)
                .expect("Fix: freeing first optimizer buffer into the pool must succeed.");
            assert_eq!(
                backend.resident_allocated_bytes(),
                baseline + 128,
                "Fix: one active buffer plus one pooled buffer should remain resident."
            );

            dispatcher
                .free_resident(second)
                .expect("Fix: freeing second optimizer buffer must respect the pool budget.");
            assert_eq!(
                backend.resident_allocated_bytes(),
                baseline + 64,
                "Fix: optimizer resident pool must evict excess idle buffers instead of pinning unbounded VRAM."
            );
        }

        assert_eq!(
            backend.resident_allocated_bytes(),
            baseline,
            "Fix: dropping the optimizer dispatcher must release every retained resident buffer."
        );
    }

    #[test]
    fn cuda_optimizer_resident_pool_accounting_is_exact_not_saturating() {
        let source = include_str!("optimizer.rs");
        assert!(
            !source.contains(concat!("pooled_bytes", ".saturating_add"))
                && !source.contains(concat!("pooled_bytes", ".saturating_sub"))
                && !source.contains(concat!(".saturating_add", "(incoming_bytes)")),
            "Fix: CUDA optimizer resident pool byte accounting must be exact; saturation hides VRAM pressure bugs."
        );
        assert!(
            !source.contains(concat!(
                ".expect(\"",
                "CUDA optimizer resident pool byte accounting underflowed during reuse"
            )),
            "Fix: CUDA optimizer resident pool accounting must return a typed DispatchError instead of panicking during reuse."
        );
        assert!(
            !source.contains(concat!(
                ".expect(\"",
                "CUDA optimizer resident pool byte accounting underflowed during eviction"
            )),
            "Fix: CUDA optimizer resident pool accounting must return a typed DispatchError instead of panicking during eviction."
        );
        assert!(
            !source.contains(concat!(
                ".expect(\"",
                "CUDA optimizer resident pool byte accounting overflowed while pooling a handle"
            )),
            "Fix: CUDA optimizer resident pool accounting must return a typed DispatchError instead of panicking during pooling."
        );
        assert!(
            source.contains("fn reserve_optimizer_vec<T>(")
                && !source.contains(concat!("Vec::with_capacity", "(ids.len())"))
                && !source.contains(concat!("Vec::with_capacity", "(uploads.len())"))
                && !source.contains(concat!("Vec::with_capacity", "(ranges.len())"))
                && !source.contains(concat!("Vec::with_capacity", "(steps.len())"))
                && !source.contains(concat!("Vec::with_capacity", "(read_ranges.len())")),
            "Fix: CUDA optimizer resident staging must reserve fallibly before sequence/readback fan-out growth."
        );
        assert!(
            source.contains("use crate::numeric::CUDA_NUMERIC;")
                && source.contains(".usize_to_u64(value, label)")
                && !source.contains(concat!("u64::try_from", "(value)")),
            "Fix: CUDA optimizer telemetry/static-cache sizes must use the shared backend numeric policy."
        );
        assert!(
            source.contains("domain_separated_exact_input_key")
                && source.contains("ExactInputKey")
                && !source.contains(&["blake", "3::Hasher::new()"].concat()),
            "Fix: CUDA optimizer static upload cache identity must use the shared domain-separated exact-input key instead of forking BLAKE3 tuple hashing."
        );
    }

    #[test]
    fn cuda_optimizer_static_upload_cache_skips_warm_h2d_and_releases_on_drop() {
        let backend = CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
        let baseline = backend.resident_allocated_bytes();
        let payload_a = vec![0xA5_u8; 256];
        let payload_b = vec![0x5A_u8; 512];
        let expected_h2d = (payload_a.len() + payload_b.len()) as u64;

        {
            let dispatcher =
                CudaOptimizerDispatcher::new_with_pool_budget_for_tests(&backend, expected_h2d * 4);
            backend.reset_telemetry();
            let cold = dispatcher
                .acquire_resident_static_uploads(0x4355_4441_5354_4154, &[&payload_a, &payload_b])
                .expect("Fix: cold static resident upload must allocate and upload.");
            assert!(
                !cold.cache_hit,
                "Fix: first static resident acquisition cannot claim a cache hit."
            );
            assert!(
                cold.retained_by_dispatcher,
                "Fix: cacheable CUDA static resident handles must stay owned by the dispatcher."
            );
            dispatcher
                .release_resident_static_uploads(cold)
                .expect("Fix: releasing a retained static resident set must be a no-op.");
            assert_eq!(
                backend.telemetry_snapshot().host_to_device_bytes,
                expected_h2d,
                "Fix: cold static resident acquisition must upload each immutable payload exactly once."
            );

            backend.reset_telemetry();
            let warm = dispatcher
                .acquire_resident_static_uploads(0x4355_4441_5354_4154, &[&payload_a, &payload_b])
                .expect("Fix: warm static resident acquisition must reuse device buffers.");
            assert!(
                warm.cache_hit,
                "Fix: identical immutable CUDA payloads must be served from resident cache."
            );
            dispatcher
                .release_resident_static_uploads(warm)
                .expect("Fix: releasing a warm retained static resident set must be a no-op.");
            assert_eq!(
                backend.telemetry_snapshot().host_to_device_bytes,
                0,
                "Fix: warm static resident acquisition must not re-upload immutable payloads."
            );
        }

        assert_eq!(
            backend.resident_allocated_bytes(),
            baseline,
            "Fix: dropping the CUDA optimizer dispatcher must release retained static-cache buffers."
        );
    }

    #[test]
    fn cuda_optimizer_resident_clear_uses_device_memset_not_h2d_upload() {
        let backend = CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
        let dispatcher = CudaOptimizerDispatcher::new_with_pool_budget_for_tests(&backend, 4096);
        let handle = dispatcher
            .alloc_resident(64)
            .expect("Fix: resident clear test allocation must succeed.");
        dispatcher
            .upload_resident(handle, &[0xFF_u8; 64])
            .expect("Fix: resident clear test seed upload must succeed.");

        backend.reset_telemetry();
        let mut outputs = Vec::new();
        dispatcher
            .clear_upload_resident_many_sequence_read_many_into(
                &[(handle, 64)],
                &[],
                &[],
                &[handle],
                &mut outputs,
            )
            .expect("Fix: CUDA resident clear+read sequence must succeed.");
        assert_eq!(
            backend.telemetry_snapshot().host_to_device_bytes,
            0,
            "Fix: CUDA resident clears must use device memset instead of H2D zero uploads."
        );
        assert_eq!(
            outputs,
            vec![vec![0_u8; 64]],
            "Fix: CUDA resident clear must zero every byte before readback."
        );

        backend.reset_telemetry();
        dispatcher
            .fill_upload_resident_many_sequence_read_many_into(
                &[(handle, 64, 0xA5)],
                &[],
                &[],
                &[handle],
                &mut outputs,
            )
            .expect("Fix: CUDA resident fill+read sequence must succeed.");
        assert_eq!(
            backend.telemetry_snapshot().host_to_device_bytes,
            0,
            "Fix: CUDA resident fills must use device memset instead of H2D byte-pattern uploads."
        );
        assert_eq!(
            outputs,
            vec![vec![0xA5_u8; 64]],
            "Fix: CUDA resident fill must write the requested byte pattern before readback."
        );

        dispatcher
            .free_resident(handle)
            .expect("Fix: resident clear test handle must return to the pool.");
    }
}

fn cuda_optimizer_resident_pool_budget_bytes(total_memory_bytes: u64) -> u64 {
    total_memory_bytes / CUDA_OPTIMIZER_RESIDENT_POOL_BUDGET_DENOMINATOR
}

impl<'a> Drop for CudaOptimizerDispatcher<'a> {
    fn drop(&mut self) {
        self.drain_static_upload_cache();
        self.drain_pool();
    }
}

impl<'a> OptimizerDispatcher for CudaOptimizerDispatcher<'a> {
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let mut config = DispatchConfig::default();
        config.grid_override = grid_override;
        // CudaBackend's borrowed-dispatch path is what `dispatch` was
        // routing through previously. Keep parity for callers that
        // don't want the persistent fast-path.
        self.backend
            .dispatch(program, inputs, &config)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn supports_persistent(&self) -> bool {
        true
    }

    fn device_feature_cache_key(&self) -> u64 {
        (u64::from(self.backend.ptx_target_sm()) << 32)
            | u64::from(self.backend.pipeline_feature_flags().bits())
    }

    fn alloc_resident(&self, byte_len: usize) -> Result<u64, DispatchError> {
        // Try the pool first. The size-class lookup is exact: a
        // handle of `byte_len = 4096` is NOT pulled for a request of
        // `byte_len = 2048` even though it would fit, because the
        // backend's static-size verifier checks
        // `resident.byte_len >= expected` and the kernel's binding
        // contract assumes the buffer is of the declared length, not
        // larger. Exact-match keeps the pool semantics safe.
        if let Some(handles) = self.free_pool.borrow_mut().get_mut(&byte_len) {
            if let Some(handle) = handles.pop() {
                {
                    let mut pooled_bytes = self.pooled_bytes.borrow_mut();
                    let handle_bytes = optimizer_usize_to_u64(
                        handle.byte_len,
                        "resident pool reused handle bytes",
                    )?;
                    *pooled_bytes = pooled_bytes.checked_sub(handle_bytes).ok_or_else(|| {
                        DispatchError::BackendError(
                            "CUDA optimizer resident pool byte accounting underflowed during reuse"
                                .to_string(),
                        )
                    })?;
                }
                self.sizes.borrow_mut().insert(handle.id, handle.byte_len);
                return Ok(handle.id);
            }
        }
        let handle = self
            .backend
            .allocate_resident(byte_len)
            .map_err(|e| DispatchError::BackendError(e.to_string()))?;
        self.sizes.borrow_mut().insert(handle.id, handle.byte_len);
        Ok(handle.id)
    }

    fn upload_resident(&self, id: u64, bytes: &[u8]) -> Result<(), DispatchError> {
        let handle = self.resolve(id)?;
        self.backend
            .upload_resident(handle, bytes)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn upload_resident_many(&self, uploads: &[(u64, &[u8])]) -> Result<(), DispatchError> {
        let concrete = self.resolve_uploads(uploads)?;
        self.backend
            .upload_resident_many(&concrete)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn acquire_resident_static_uploads(
        &self,
        cache_domain: u64,
        payloads: &[&[u8]],
    ) -> Result<ResidentStaticBufferSet, DispatchError> {
        let key = self.static_upload_cache_key(cache_domain, payloads)?;
        if let Some(entry) = self.static_upload_cache.borrow().get(&key) {
            let mut handles = Vec::new();
            reserve_optimizer_vec(
                &mut handles,
                entry.handles.len(),
                "optimizer cached static handles",
            )?;
            for handle in &entry.handles {
                handles.push(handle.id);
            }
            return Ok(ResidentStaticBufferSet {
                handles,
                cache_hit: true,
                retained_by_dispatcher: true,
            });
        }

        let mut handles = Vec::new();
        reserve_optimizer_vec(&mut handles, payloads.len(), "optimizer static handles")?;
        for payload in payloads {
            match self.alloc_resident(payload.len()) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    for handle in handles.iter().copied() {
                        let _ = self.free_resident(handle);
                    }
                    return Err(error);
                }
            }
        }

        let mut uploads = Vec::new();
        reserve_optimizer_vec(&mut uploads, payloads.len(), "optimizer static uploads")?;
        for (&handle, &payload) in handles.iter().zip(payloads.iter()) {
            uploads.push((handle, payload));
        }
        if let Err(error) = self.upload_resident_many(&uploads) {
            for handle in handles.iter().copied() {
                let _ = self.free_resident(handle);
            }
            return Err(error);
        }

        let bytes = self.static_payload_bytes(payloads)?;
        if !self.evict_until_static_upload_cache_has_room(bytes)? {
            return Ok(ResidentStaticBufferSet {
                handles,
                cache_hit: false,
                retained_by_dispatcher: false,
            });
        }

        let mut cached_handles = Vec::new();
        reserve_optimizer_vec(
            &mut cached_handles,
            handles.len(),
            "optimizer static cached concrete handles",
        )?;
        for &handle in &handles {
            cached_handles.push(self.resolve(handle)?);
        }
        self.static_upload_cache.borrow_mut().insert(
            key,
            StaticUploadCacheEntry {
                handles: cached_handles,
                bytes,
            },
        );
        {
            let mut cached_bytes = self.static_cached_bytes.borrow_mut();
            *cached_bytes = checked_add_u64_lazy(*cached_bytes, bytes, || {
                DispatchError::BackendError(
                    "CUDA optimizer static cache byte accounting overflowed while inserting"
                        .to_string(),
                )
            })?;
        }
        Ok(ResidentStaticBufferSet {
            handles,
            cache_hit: false,
            retained_by_dispatcher: true,
        })
    }

    fn release_resident_static_uploads(
        &self,
        set: ResidentStaticBufferSet,
    ) -> Result<(), DispatchError> {
        if set.retained_by_dispatcher {
            return Ok(());
        }
        for handle in set.handles {
            self.free_resident(handle)?;
        }
        Ok(())
    }

    fn read_resident(&self, id: u64) -> Result<Vec<u8>, DispatchError> {
        let handle = self.resolve(id)?;
        self.backend
            .download_resident(handle)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn read_resident_many(&self, ids: &[u64]) -> Result<Vec<Vec<u8>>, DispatchError> {
        let handles = self.resolve_many(ids)?;
        self.backend
            .download_resident_many(&handles)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn read_resident_ranges(
        &self,
        ranges: &[ResidentReadRange],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let (handles, readbacks) = self.resolve_read_ranges(ranges)?;
        self.backend
            .download_resident_readbacks_many(&handles, &readbacks)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn free_resident(&self, id: u64) -> Result<(), DispatchError> {
        let handle = self.resolve(id)?;
        // Don't actually free; return the handle to the pool. Exact
        // size-class push so the next `alloc_resident(byte_len)` of
        // the same size can pop in O(1). The handle id stays in
        // `free_pool` until exact-size reuse or budget eviction.
        self.sizes.borrow_mut().remove(&id);
        let handle_bytes =
            optimizer_usize_to_u64(handle.byte_len, "resident pool freed handle bytes")?;
        if !self.evict_until_resident_pool_has_room(handle_bytes)? {
            self.backend
                .free_resident(handle)
                .map_err(|e| DispatchError::BackendError(e.to_string()))?;
            return Ok(());
        }
        self.free_pool
            .borrow_mut()
            .entry(handle.byte_len)
            .or_default()
            .push(handle);
        {
            let mut pooled_bytes = self.pooled_bytes.borrow_mut();
            *pooled_bytes = checked_add_u64_lazy(*pooled_bytes, handle_bytes, || {
                DispatchError::BackendError(
                    "CUDA optimizer resident pool byte accounting overflowed while pooling a handle"
                        .to_string(),
                )
            })?;
        }
        Ok(())
    }

    fn dispatch_resident(
        &self,
        program: &Program,
        handle_ids: &[u64],
        grid_override: Option<[u32; 3]>,
    ) -> Result<(), DispatchError> {
        let handles = self.resolve_many(handle_ids)?;
        let mut config = DispatchConfig::default();
        config.grid_override = grid_override;
        // `CudaBackend::dispatch_resident` does NOT auto-readback; that
        // is what makes the persistent path fast. Caller invokes
        // `read_resident` only at the end of the pipeline.
        self.backend
            .dispatch_resident(program, &handles, &config)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn dispatch_resident_sequence(
        &self,
        steps: &[ResidentDispatchStep<'_>],
    ) -> Result<(), DispatchError> {
        let resolved_handles = self.resolve_step_handles(steps, "optimizer sequence handles")?;
        let cuda_steps =
            self.build_cuda_steps(steps, &resolved_handles, "optimizer sequence step")?;
        self.backend
            .dispatch_resident_sequence(&cuda_steps)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn dispatch_resident_sequence_read_many(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer read sequence handles")?;
        let resolved_reads = self.resolve_many(read_handles)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer read sequence step",
        )?;
        self.backend
            .dispatch_resident_sequence_read_many(&cuda_steps, &resolved_reads)
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn upload_resident_many_sequence_read_many(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let concrete_uploads = self.resolve_uploads(uploads)?;
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer upload-read sequence handles")?;
        let resolved_reads = self.resolve_many(read_handles)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer upload-read sequence step",
        )?;
        self.backend
            .upload_resident_many_sequence_read_many(
                &concrete_uploads,
                &cuda_steps,
                &resolved_reads,
            )
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn upload_resident_many_sequence_read_ranges(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let mut outputs = Vec::new();
        reserve_optimizer_vec(&mut outputs, read_ranges.len(), "optimizer range output")?;
        self.upload_resident_many_sequence_read_ranges_into(
            uploads,
            steps,
            read_ranges,
            &mut outputs,
        )?;
        Ok(outputs)
    }

    fn upload_resident_many_sequence_read_many_into(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let concrete_uploads = self.resolve_uploads(uploads)?;
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer upload-read-into sequence handles")?;
        let resolved_reads = self.resolve_many(read_handles)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer upload-read-into sequence step",
        )?;
        self.backend
            .upload_resident_many_sequence_read_many_into(
                &concrete_uploads,
                &cuda_steps,
                &resolved_reads,
                outputs,
            )
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn clear_upload_resident_many_sequence_read_many_into(
        &self,
        clears: &[(u64, usize)],
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let concrete_clears = self.resolve_clears(clears)?;
        let concrete_uploads = self.resolve_uploads(uploads)?;
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer clear-upload-read sequence handles")?;
        let resolved_reads = self.resolve_many(read_handles)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer clear-upload-read sequence step",
        )?;
        self.backend
            .clear_upload_resident_many_sequence_read_many_into(
                &concrete_clears,
                &concrete_uploads,
                &cuda_steps,
                &resolved_reads,
                outputs,
            )
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn fill_upload_resident_many_sequence_read_many_into(
        &self,
        fills: &[(u64, usize, u8)],
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let concrete_fills = self.resolve_fills(fills)?;
        let concrete_uploads = self.resolve_uploads(uploads)?;
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer fill-upload-read sequence handles")?;
        let resolved_reads = self.resolve_many(read_handles)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer fill-upload-read sequence step",
        )?;
        self.backend
            .fill_upload_resident_many_sequence_read_many_into(
                &concrete_fills,
                &concrete_uploads,
                &cuda_steps,
                &resolved_reads,
                outputs,
            )
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn fill_upload_resident_many_sequence_read_ranges_into(
        &self,
        fills: &[(u64, usize, u8)],
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let concrete_fills = self.resolve_fills(fills)?;
        let concrete_uploads = self.resolve_uploads(uploads)?;
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer fill-upload-range sequence handles")?;
        let (resolved_reads, concrete_readbacks) = self.resolve_read_ranges(read_ranges)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer fill-upload-range sequence step",
        )?;
        if outputs.len() < read_ranges.len() {
            outputs.resize_with(read_ranges.len(), Vec::new);
        } else {
            outputs.truncate(read_ranges.len());
        }
        let mut borrowed_outputs = Vec::new();
        reserve_optimizer_vec(
            &mut borrowed_outputs,
            outputs.len(),
            "optimizer fill-upload-range borrowed output",
        )?;
        borrowed_outputs.extend(outputs.iter_mut());
        self.backend
            .fill_upload_resident_many_sequence_read_ranges_borrowed_into(
                &concrete_fills,
                &concrete_uploads,
                &cuda_steps,
                &resolved_reads,
                &concrete_readbacks,
                borrowed_outputs.as_mut_slice(),
            )
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }

    fn upload_resident_many_sequence_read_ranges_into(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let concrete_uploads = self.resolve_uploads(uploads)?;
        let resolved_step_handles =
            self.resolve_step_handles(steps, "optimizer upload-range sequence handles")?;
        let (resolved_reads, concrete_readbacks) = self.resolve_read_ranges(read_ranges)?;
        let cuda_steps = self.build_cuda_steps(
            steps,
            &resolved_step_handles,
            "optimizer upload-range sequence step",
        )?;
        self.backend
            .upload_resident_many_sequence_read_ranges_into(
                &concrete_uploads,
                &cuda_steps,
                &resolved_reads,
                &concrete_readbacks,
                outputs,
            )
            .map_err(|e| DispatchError::BackendError(e.to_string()))
    }
}

fn optimizer_usize_to_u64(value: usize, label: &'static str) -> Result<u64, DispatchError> {
    CUDA_NUMERIC
        .usize_to_u64(value, label)
        .map_err(|error| DispatchError::BackendError(error.to_string()))
}
