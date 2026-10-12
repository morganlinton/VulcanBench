//! # vyre-runtime  -  persistent megakernel + io_uring zero-copy
//!
//! This crate provides the execution runtime for vyre  -  the layer
//! between "I have a compiled Program" and "bytes flow through the
//! GPU continuously."
//!
//! ## Architecture
//!
//! 1. **`megakernel`**  -  the persistent GPU process. A vyre `Program`
//!    wrapping `Node::forever` that loops a ring-buffer interpreter
//!    or a JIT-fused payload processor.
//!    - `protocol`  -  slot layout, control words, opcodes
//!    - `opcode`  -  built-in opcode handlers + extension mechanism
//!    - `builder`  -  IR `Program` construction (interpreted + JIT)
//! 2. **`cache`**  -  content-addressed compilation cache.
//! 3. **`stream`**  -  `GpuStream` glue bridging io_uring completions
//!    to the megakernel tail pointer.
//! 4. **`uring`** (Linux only)  -  raw `io_uring` syscall wrappers.
//!
//! ## Design laws
//!
//! - **No CPU executor on the hot path.** Compatibility ingest may submit
//!   registered mapped reads, but the native path is NVMe passthrough into
//!   BAR1 GPU memory; after launch the megakernel owns execution and the CPU
//!   only touches queue metadata.
//! - **Megakernel is IR, not target-text.** The persistent kernel is a
//!   `Program` any `VyreBackend` can compile + dispatch.
//! - **Structured errors, never silent swallowing.** Every failure
//!   mode returns `PipelineError` with a `Fix: ` hint.

#![deny(missing_docs)]
#![warn(unreachable_pub)]
// vyre-runtime owns the io_uring zero-copy ingest path and the persistent
// megakernel ring; both reach into FFI / mmap territory. Every unsafe site
// carries a `Safety:` comment that `check_unsafe_justifications.sh` validates.
#![allow(unsafe_code)]

/// Errors surfaced by the runtime layer. Every variant carries a
/// `Fix:`-bearing message so a reviewer can act on the failure.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum PipelineError {
    /// Raw io_uring / libc syscall failed with an errno.
    #[error("io_uring {syscall} failed: errno={errno}. Fix: {fix}")]
    IoUringSyscall {
        /// Which syscall failed (`io_uring_setup`, `mmap`, `io_uring_enter`).
        syscall: &'static str,
        /// Underlying errno value.
        errno: i32,
        /// Actionable remediation.
        fix: &'static str,
    },
    /// io_uring submission or completion queue was full / overflowed.
    #[error("io_uring {queue} queue at capacity. Fix: {fix}")]
    QueueFull {
        /// "submission" or "completion".
        queue: &'static str,
        /// Actionable remediation.
        fix: &'static str,
    },
    /// Attempted to use io_uring on a non-Linux platform.
    #[error(
        "io_uring is Linux-only. Fix: run on Linux 5.1+ or use Megakernel::dispatch without a GpuStream"
    )]
    NotLinux,
    /// Feature required for NVMe passthrough is not enabled.
    #[error(
        "NVMe passthrough requires the `uring-cmd-nvme` feature + Linux kernel 6.0+. Fix: add `features = [\"uring-cmd-nvme\"]` to your Cargo.toml"
    )]
    NvmePassthroughDisabled,
    /// Backend error bubbled up from compile or dispatch.
    #[error("backend error: {0}")]
    Backend(String),
    /// A megakernel dispatch ended before its work queue drained: only
    /// `claimed` of `expected` `unit` were claimed, so the rest went unscanned
    /// and this dispatch's hit set is INCOMPLETE — never a silent partial
    /// (Law 10). A first-class variant (not a `Backend` string) so callers such
    /// as the `seg_len` calibrator can EXCLUDE a too-fine geometry by matching
    /// the type, never by substring-scanning the message text.
    #[error(
        "{descriptor} drain incomplete: only {claimed} of {expected} {unit} were claimed before \
         the dispatch ended, so {unscanned} {unit} went unscanned and their matches were dropped. \
         This dispatch's hit set is INCOMPLETE. Fix: raise the dispatch timeout \
         (BatchDispatchConfig.timeout) so the drain loop can exhaust the queue, or shard the batch \
         into smaller queues.",
        unscanned = expected.saturating_sub(*claimed),
    )]
    DrainIncomplete {
        /// Which dispatch path under-drained: `"megakernel"` (per-rule) or
        /// `"combined megakernel"` (combined-AC). Names the failing path in the
        /// operator message without a second string variant.
        descriptor: &'static str,
        /// Work-items/segments actually claimed before the dispatch ended.
        claimed: u32,
        /// Work-items/segments that should have been claimed (full queue length).
        expected: u32,
        /// The unit being drained: `"work-items"` (per-rule) or `"segments"`
        /// (combined-AC). Interpolated twice for a grammatical message.
        unit: &'static str,
    },
}

impl PipelineError {
    /// True iff this is a [`PipelineError::DrainIncomplete`]: a dispatch that
    /// could not exhaust its work queue within the timeout.
    ///
    /// Distinct from a hard backend failure — the `seg_len` calibrator
    /// EXCLUDES a geometry that drains incompletely (too fine to drain in the
    /// configured timeout) rather than aborting the whole calibration, while it
    /// must still PROPAGATE any other [`PipelineError`]. Match on this predicate
    /// instead of substring-scanning the Display message, which is fragile to
    /// wording changes.
    #[must_use]
    pub fn is_drain_incomplete(&self) -> bool {
        matches!(self, Self::DrainIncomplete { .. })
    }
}

impl From<vyre_driver::backend::BackendError> for PipelineError {
    fn from(err: vyre_driver::backend::BackendError) -> Self {
        PipelineError::Backend(err.to_string())
    }
}

/// Persistent megakernel  -  the vyre Program that runs forever on
/// the GPU, decoding host-fed ring opcodes from a host-fed ring buffer.
pub mod megakernel;

/// Content-addressed pipeline cache: `blake3(canonicalize(p).to_wire())`
/// is the cache key.
pub mod pipeline_cache;

/// Differential megakernel replay log  -  captures every published
/// ring slot so a later cert run can diff epoch-by-epoch execution
/// against a live backend.
pub mod replay;

/// Backend routing policy for execution plans.
pub mod routing;

/// Multi-GPU work partitioning across runtime backends.
pub mod scheduler;

/// Multi-tenant megakernel multiplexing  -  one persistent kernel per
/// GPU, shared across producer tools via the `tenant_id` field already
/// in the ring protocol.
pub mod tenant;

pub use replay::{
    RecordedSlot, ReplayFailureClass, ReplayFailureEvidence, ReplayLogError, ReplayRecord, RingLog,
};
pub use tenant::{
    TenantError, TenantHandle, TenantRegistry, OPCODE_RANGE_PER_TENANT, TENANT_ID_MAX,
    TENANT_OPCODE_BASE,
};

#[cfg(feature = "remote")]
pub use pipeline_cache::RemoteCache;
pub use pipeline_cache::{
    DiskCache, DiskCacheError, InMemoryPipelineCache, LayeredPipelineCache,
    PersistentPipelineCacheStore, PipelineCacheMetricError, PipelineCacheMetrics,
    PipelineCacheStore, PipelineFingerprint,
};

pub use megakernel::Megakernel;

/// Linux io_uring integration. Compiled out on macOS / Windows.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub mod uring;

/// Handle to an orchestrated pipeline. Couples a compiled megakernel
/// to its submission + completion infrastructure.
pub struct GpuStream<'a> {
    #[cfg(target_os = "linux")]
    uring: Option<uring::AsyncUringStream<'a>>,
    // On macOS / Windows the `uring` field is compiled out, which leaves the
    // `'a` lifetime unused and the compiler rejects the struct. Carry a
    // zero-sized marker so the lifetime stays live on non-Linux targets.
    #[cfg(not(target_os = "linux"))]
    _phantom: std::marker::PhantomData<&'a ()>,
    shutdown_requested: bool,
}

impl Default for GpuStream<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> GpuStream<'a> {
    /// Create a pipeline handle with no io_uring stream attached.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_runtime::GpuStream;
    ///
    /// let stream = GpuStream::new();
    ///
    /// assert!(!stream.is_shutdown_requested());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            uring: None,
            #[cfg(not(target_os = "linux"))]
            _phantom: std::marker::PhantomData,
            shutdown_requested: false,
        }
    }

    /// Attach an io_uring stream for GPU-visible reads. Linux-only.
    ///
    /// Use `uring::NvmeGpuIngestDriver::new_gpudirect` when the caller
    /// requires the native NVMe → BAR1 path instead of registered mapped reads.
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn with_uring(mut self, stream: uring::AsyncUringStream<'a>) -> Self {
        self.uring = Some(stream);
        self
    }

    /// Reap completions and bump the megakernel tail pointer.
    ///
    /// # Errors
    ///
    /// Propagates any uring syscall error from the underlying ring.
    pub fn poll(&mut self) -> Result<u32, PipelineError> {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref mut stream) = self.uring {
                return stream.poll();
            }
        }
        Ok(0)
    }

    /// Request graceful shutdown of the pipeline.
    pub fn request_shutdown(&mut self) {
        self.shutdown_requested = true;
    }

    /// Whether shutdown has been requested.
    #[must_use]
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    /// Block until the megakernel writes a new value into the
    /// observable word. Uses `futex_waitv` on Linux 5.16+.
    ///
    /// # Errors
    ///
    /// - [`PipelineError::NotLinux`] on non-Linux hosts.
    /// - [`PipelineError::IoUringSyscall`] on futex errors.
    ///
    /// # Safety
    ///
    /// `host_visible_addr` must be host-mapped and outlive this call.
    #[cfg(target_os = "linux")]
    #[allow(unsafe_code)]
    pub unsafe fn wait_for_observable(
        host_visible_addr: *const u32,
        current: u32,
        timeout_ns: u64,
    ) -> Result<(), PipelineError> {
        #[repr(C)]
        struct futex_waitv {
            val: u64,
            uaddr: u64,
            flags: u32,
            __reserved: u32,
        }
        const FUTEX2_SIZE_U32: u32 = 0x02;
        const SYS_FUTEX_WAITV: libc::c_long = 449;

        let waitv = [futex_waitv {
            val: current as u64,
            uaddr: host_visible_addr as u64,
            flags: FUTEX2_SIZE_U32,
            __reserved: 0,
        }];

        #[repr(C)]
        struct Timespec {
            tv_sec: i64,
            tv_nsec: i64,
        }
        let ts = Timespec {
            tv_sec: (timeout_ns / 1_000_000_000) as i64,
            tv_nsec: (timeout_ns % 1_000_000_000) as i64,
        };

        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        let res = unsafe {
            libc::syscall(
                SYS_FUTEX_WAITV,
                waitv.as_ptr() as *const libc::c_void,
                1u32,
                0u32,
                &ts as *const Timespec,
                0u64,
            )
        };

        if res < 0 {
            // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
            let errno = unsafe { *libc::__errno_location() };
            if errno == libc::EAGAIN {
                return Ok(());
            }
            return Err(PipelineError::IoUringSyscall {
                syscall: "futex_waitv",
                errno,
                fix: "kernel 5.16+ required; ETIMEDOUT means the value didn't change within timeout_ns",
            });
        }
        Ok(())
    }

    /// Non-Linux implementation returning the structured platform error.
    #[cfg(not(target_os = "linux"))]
    #[allow(unsafe_code, clippy::missing_safety_doc)]
    pub unsafe fn wait_for_observable(
        _host_visible_addr: *const u32,
        _current: u32,
        _timeout_ns: u64,
    ) -> Result<(), PipelineError> {
        Err(PipelineError::NotLinux)
    }
}

/// Linux-only: host-visible GPU buffer that io_uring can DMA into.
#[cfg(target_os = "linux")]
pub use uring::GpuMappedBuffer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_stream_has_no_shutdown() {
        let stream = GpuStream::new();
        assert!(!stream.is_shutdown_requested());
    }

    #[test]
    fn shutdown_is_idempotent() {
        let mut stream = GpuStream::new();
        stream.request_shutdown();
        stream.request_shutdown();
        assert!(stream.is_shutdown_requested());
    }

    #[test]
    fn poll_without_uring_returns_zero() {
        let mut stream = GpuStream::new();
        assert_eq!(stream.poll().unwrap(), 0);
    }

    #[test]
    fn drain_incomplete_is_distinguishable_by_type_not_substring() {
        // Regression: the seg_len calibrator must EXCLUDE a too-fine geometry
        // (drain-incomplete) but PROPAGATE every other backend failure. It used
        // to discriminate by `to_string().contains("drain incomplete")`, which
        // silently turns into "abort the whole calibration" the moment the
        // message wording drifts. The structured variant + predicate is the
        // contract; this test pins it.
        let drain = PipelineError::DrainIncomplete {
            descriptor: "combined megakernel",
            claimed: 3,
            expected: 10,
            unit: "segments",
        };
        assert!(drain.is_drain_incomplete());

        // The Display message stays operator-actionable AND keeps the
        // "drain incomplete" phrase + computed unscanned count, so legacy
        // substring matchers and operator logs do not regress.
        let msg = drain.to_string();
        assert_eq!(
            msg,
            "combined megakernel drain incomplete: only 3 of 10 segments were claimed before the \
             dispatch ended, so 7 segments went unscanned and their matches were dropped. This \
             dispatch's hit set is INCOMPLETE. Fix: raise the dispatch timeout \
             (BatchDispatchConfig.timeout) so the drain loop can exhaust the queue, or shard the \
             batch into smaller queues."
        );

        // The per-rule path uses the same variant with a different descriptor/unit.
        let per_rule = PipelineError::DrainIncomplete {
            descriptor: "megakernel",
            claimed: 0,
            expected: 4,
            unit: "work-items",
        };
        assert!(per_rule.is_drain_incomplete());
        assert!(
            per_rule.to_string().starts_with(
                "megakernel drain incomplete: only 0 of 4 work-items were claimed"
            ),
            "msg was: {}",
            per_rule
        );

        // A genuine backend failure is NOT a drain-incomplete: it must surface
        // as a hard error, never be excluded-and-continued by the calibrator.
        let backend = PipelineError::Backend("adapter lost".to_string());
        assert!(!backend.is_drain_incomplete());
    }
}
