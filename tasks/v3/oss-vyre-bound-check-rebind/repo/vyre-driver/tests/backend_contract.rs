//! Contract tests for the frozen `VyreBackend` trait surface (vyre 0.6).
//!
//! Two fixtures:
//!
//! - `MinimalBackend` overrides nothing beyond `id` + `dispatch`. Every
//!   capability query and lifecycle hook takes the defaulted answer
//!   (conservative "no" / minimal-limit / Ok(())). This is the
//!   "new backend added with minimum viable code" shape  -  if a
//!   defaulted method ever gets renamed, moved, or made non-default,
//!   this test stops compiling.
//! - `FullBackend` overrides every capability query and every
//!   lifecycle hook. Each override returns a value observably
//!   different from the conservative default. This confirms the
//!   overrides reach the trait dispatch correctly and guards against
//!   accidental removal of an override slot.
//!
//! Together they freeze the default ⇄ override contract for 0.6.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use vyre_driver::{BackendError, DispatchConfig, VyreBackend};
use vyre_foundation::ir::Program;

/// Minimum-viable backend: every capability + lifecycle query takes
/// the default body.
struct MinimalBackend;

impl vyre_driver::backend::private::Sealed for MinimalBackend {}

impl VyreBackend for MinimalBackend {
    fn id(&self) -> &'static str {
        "minimal"
    }

    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        Ok(vec![])
    }
}

/// Maximal-capability backend: every query + hook overridden.
///
/// Records lifecycle-hook calls via atomics so a test can assert each
/// hook actually ran.
struct FullBackend {
    prepare_calls: AtomicUsize,
    flush_calls: AtomicUsize,
    shutdown_calls: AtomicUsize,
    recover_calls: AtomicUsize,
}

impl FullBackend {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            prepare_calls: AtomicUsize::new(0),
            flush_calls: AtomicUsize::new(0),
            shutdown_calls: AtomicUsize::new(0),
            recover_calls: AtomicUsize::new(0),
        })
    }
}

impl vyre_driver::backend::private::Sealed for FullBackend {}

impl VyreBackend for FullBackend {
    fn id(&self) -> &'static str {
        "full"
    }
    fn version(&self) -> &'static str {
        "0.6.0-test"
    }
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        Ok(vec![])
    }

    // Capability queries  -  everything flipped from the conservative
    // default.
    fn supports_subgroup_ops(&self) -> bool {
        true
    }
    fn supports_f16(&self) -> bool {
        true
    }
    fn supports_bf16(&self) -> bool {
        true
    }
    fn supports_tensor_cores(&self) -> bool {
        true
    }
    fn supports_async_compute(&self) -> bool {
        true
    }
    fn supports_indirect_dispatch(&self) -> bool {
        true
    }
    fn is_distributed(&self) -> bool {
        true
    }
    fn max_workgroup_size(&self) -> [u32; 3] {
        [1024, 1024, 64]
    }
    fn max_storage_buffer_bytes(&self) -> u64 {
        1 << 40
    }

    // Lifecycle hooks  -  record each call so tests can verify dispatch
    // reaches the override.
    fn prepare(&self) -> Result<(), BackendError> {
        self.prepare_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn flush(&self) -> Result<(), BackendError> {
        self.flush_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn shutdown(&self) -> Result<(), BackendError> {
        self.shutdown_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn device_lost(&self) -> bool {
        true
    }
    fn try_recover(&self) -> Result<(), BackendError> {
        self.recover_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn minimal_backend_defaults_are_conservative() {
    let backend = MinimalBackend;
    assert!(
        !backend.supports_subgroup_ops(),
        "default supports_subgroup_ops must be false"
    );
    assert!(
        !backend.supports_f16(),
        "default supports_f16 must be false"
    );
    assert!(
        !backend.supports_bf16(),
        "default supports_bf16 must be false"
    );
    assert!(
        !backend.supports_tensor_cores(),
        "default supports_tensor_cores must be false"
    );
    assert!(
        !backend.supports_async_compute(),
        "default supports_async_compute must be false"
    );
    assert!(
        !backend.supports_indirect_dispatch(),
        "default supports_indirect_dispatch must be false"
    );
    assert!(
        !backend.is_distributed(),
        "default is_distributed must be false"
    );
    assert_eq!(
        backend.max_workgroup_size(),
        [1, 1, 1],
        "default max_workgroup_size must be scalar [1,1,1]"
    );
    assert_eq!(
        backend.max_storage_buffer_bytes(),
        0,
        "default max_storage_buffer_bytes must be 0"
    );
    assert!(!backend.device_lost(), "default device_lost must be false");
}

#[test]
fn minimal_backend_lifecycle_hooks_are_noops() {
    let backend = MinimalBackend;
    backend
        .prepare()
        .expect("Fix: default prepare must return Ok  -  any backend author can rely on it");
    backend.flush().expect("Fix: default flush must return Ok");
    backend
        .shutdown()
        .expect("Fix: default shutdown must return Ok");
    let err = backend
        .try_recover()
        .expect_err("default try_recover must fail - recovery is opt-in");
    assert!(
        matches!(err, BackendError::UnsupportedFeature { .. }),
        "default try_recover must return UnsupportedFeature, got: {err:?}"
    );
}

#[test]
fn full_backend_reports_maximal_capabilities() {
    let full = FullBackend::new();
    assert!(full.supports_subgroup_ops());
    assert!(full.supports_f16());
    assert!(full.supports_bf16());
    assert!(full.supports_tensor_cores());
    assert!(full.supports_async_compute());
    assert!(full.supports_indirect_dispatch());
    assert!(full.is_distributed());
    assert_eq!(full.max_workgroup_size(), [1024, 1024, 64]);
    assert_eq!(full.max_storage_buffer_bytes(), 1u64 << 40);
    assert!(full.device_lost());
}

#[test]
fn full_backend_lifecycle_hooks_fire_on_dispatch_surface() {
    let full = FullBackend::new();
    full.prepare().unwrap();
    full.flush().unwrap();
    full.shutdown().unwrap();
    full.try_recover().unwrap();
    assert_eq!(full.prepare_calls.load(Ordering::Relaxed), 1);
    assert_eq!(full.flush_calls.load(Ordering::Relaxed), 1);
    assert_eq!(full.shutdown_calls.load(Ordering::Relaxed), 1);
    assert_eq!(full.recover_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn dyn_vyre_backend_is_object_safe() {
    // If any trait addition breaks object safety, this test
    // stops compiling. Object safety is load-bearing for the trio
    // architecture: dispatch routes through `&dyn VyreBackend`.
    let _minimal: Arc<dyn VyreBackend> = Arc::new(MinimalBackend);
    let _full: Arc<dyn VyreBackend> = FullBackend::new();
}

#[test]
fn trait_version_defaults_to_unspecified() {
    let backend = MinimalBackend;
    assert_eq!(
        backend.version(),
        "unspecified",
        "backends that did not override version should report 'unspecified'"
    );
}

#[test]
fn default_dispatch_async_returns_ready_handle() {
    // Synchronous backends inherit the default dispatch_async, which
    // runs dispatch eagerly and wraps the result in a ReadyPending
    // handle. Confirms the default path works end-to-end so every
    // consumer can use the async API against every backend.
    let backend = MinimalBackend;
    let program = Program::default();
    let pending = backend
        .dispatch_async(&program, &[], &DispatchConfig::default())
        .expect("default dispatch_async must succeed for MinimalBackend");
    assert!(
        pending.is_ready(),
        "ReadyPending must report is_ready=true so poll loops exit immediately"
    );
    let outputs = pending
        .await_result()
        .expect("default dispatch_async result must be retrievable");
    assert!(
        outputs.is_empty(),
        "MinimalBackend::dispatch returns empty outputs; the async wrapper must forward verbatim"
    );
}

#[test]
fn full_backend_dispatch_async_still_works() {
    // Even a backend with every capability overridden inherits the
    // default dispatch_async unless it chooses to override. This test
    // locks in that the default remains reachable from a
    // heavily-customised impl.
    let full = FullBackend::new();
    let program = Program::default();
    let pending = full
        .dispatch_async(&program, &[], &DispatchConfig::default())
        .expect("FullBackend dispatch_async must succeed");
    assert!(pending.is_ready());
    let _ = pending.await_result().unwrap();
}
