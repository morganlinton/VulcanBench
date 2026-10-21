//! Public backend trait compatibility tests.
//!
//! Guarantees:
//! - `VyreBackend` blanket-implements `Backend` and `Executable`
//! - `CompiledPipeline`, `PendingDispatch`, `Backend`, `Executable`, `Streamable` are object-safe
//! - Default trait methods preserve backward compatibility
//! - `Send + Sync` bounds are satisfied by typical impls
//! - `Arc<dyn VyreBackend>` can call all surface methods

use std::sync::Arc;
use vyre_driver::backend::{Backend, Streamable};
use vyre_driver::{
    BackendError, CompiledPipeline, DispatchConfig, Executable, PendingDispatch, VyreBackend,
};
use vyre_foundation::ir::Program;

struct TraitProbeBackend;

impl vyre_driver::backend::private::Sealed for TraitProbeBackend {}

impl VyreBackend for TraitProbeBackend {
    fn id(&self) -> &'static str {
        "trait-probe"
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

// ------------------------------------------------------------------
// 1. Blanket impls
// ------------------------------------------------------------------

#[test]
fn vyre_backend_blanket_impls_backend() {
    let backend = TraitProbeBackend;
    let as_backend: &dyn Backend = &backend;
    assert_eq!(as_backend.id(), "trait-probe");
    assert_eq!(as_backend.version(), "unspecified");
    let _ = as_backend.supported_ops();
}

#[test]
fn executable_trait_is_implementable_by_vyre_backend() {
    // Compilation guard + functional check: a VyreBackend can also implement Executable.
    struct Probe;
    impl vyre_driver::backend::private::Sealed for Probe {}
    impl VyreBackend for Probe {
        fn id(&self) -> &'static str {
            "probe"
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
    impl Executable for Probe {
        fn dispatch(
            &self,
            program: &Program,
            inputs: &[vyre_driver::MemoryRef<'_>],
            config: &DispatchConfig,
        ) -> Result<Vec<vyre_driver::Memory>, BackendError> {
            let owned: Vec<Vec<u8>> = inputs.iter().map(|input| (*input).to_vec()).collect();
            VyreBackend::dispatch(self, program, &owned, config)
        }
    }
    let backend = Probe;
    let as_executable: &dyn Executable = &backend;
    let program = Program::default();
    let outputs = as_executable
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect("Fix: Executable dispatch must succeed");
    assert!(outputs.is_empty());
}

// ------------------------------------------------------------------
// 2. Object safety compilation guards
// ------------------------------------------------------------------

#[test]
fn compiled_pipeline_trait_is_object_safe() {
    // Compilation guard  -  if CompiledPipeline ever loses object safety this stops compiling.
    let _: Option<Box<dyn CompiledPipeline>> = None;
}

#[test]
fn pending_dispatch_trait_is_object_safe() {
    let _: Option<Box<dyn PendingDispatch>> = None;
}

#[test]
fn backend_trait_is_object_safe() {
    let _: Option<Box<dyn Backend>> = None;
}

#[test]
fn executable_trait_is_object_safe() {
    let _: Option<Box<dyn Executable>> = None;
}

#[test]
fn streamable_trait_is_object_safe() {
    let _: Option<Box<dyn Streamable>> = None;
}

// ------------------------------------------------------------------
// 3. Send + Sync bounds
// ------------------------------------------------------------------

#[test]
fn vyre_backend_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TraitProbeBackend>();
}

#[test]
fn dyn_vyre_backend_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn VyreBackend>>();
}

// ------------------------------------------------------------------
// 4. Arc<dyn VyreBackend> surface accessibility
// ------------------------------------------------------------------

#[test]
fn arc_dyn_vyre_backend_can_call_all_surface_methods() {
    let backend: Arc<dyn VyreBackend> = Arc::new(TraitProbeBackend);
    assert_eq!(backend.id(), "trait-probe");
    assert_eq!(backend.version(), "unspecified");
    assert!(!backend.supports_subgroup_ops());
    assert!(!backend.supports_f16());
    assert!(!backend.supports_bf16());
    assert!(!backend.supports_tensor_cores());
    assert!(!backend.supports_async_compute());
    assert!(!backend.supports_indirect_dispatch());
    assert!(!backend.is_distributed());
    assert_eq!(backend.max_workgroup_size(), [1, 1, 1]);
    assert_eq!(backend.max_compute_workgroups_per_dimension(), 1);
    assert_eq!(backend.max_compute_invocations_per_workgroup(), 1);
    assert_eq!(backend.max_storage_buffer_bytes(), 0);
    assert!(backend.subgroup_size().is_none());

    backend.prepare().unwrap();
    backend.flush().unwrap();
    backend.shutdown().unwrap();
    assert!(!backend.device_lost());
    assert!(backend.try_recover().is_err());
}

// ------------------------------------------------------------------
// 5. Default method contracts
// ------------------------------------------------------------------

#[test]
fn default_compile_native_returns_none() {
    let backend = TraitProbeBackend;
    let program = Program::default();
    let result = backend
        .compile_native(&program, &DispatchConfig::default())
        .expect("Fix: default compile_native must return Ok");
    assert!(
        result.is_none(),
        "Fix: default compile_native must return Ok(None)"
    );
}

#[test]
fn default_dispatch_async_returns_ready_handle() {
    let backend = TraitProbeBackend;
    let program = Program::default();
    let pending = backend
        .dispatch_async(&program, &[], &DispatchConfig::default())
        .expect("Fix: default dispatch_async must succeed");
    assert!(
        pending.is_ready(),
        "Fix: default dispatch_async must return a ready handle"
    );
    let outputs = pending
        .await_result()
        .expect("Fix: await_result must succeed");
    assert!(outputs.is_empty());
}

#[test]
fn default_dispatch_borrowed_delegates_to_dispatch() {
    let backend = TraitProbeBackend;
    let program = Program::default();
    let inputs: Vec<Vec<u8>> = vec![vec![1, 2, 3]];
    let borrowed: Vec<&[u8]> = inputs.iter().map(|v| v.as_slice()).collect();
    let outputs = backend
        .dispatch_borrowed(&program, &borrowed, &DispatchConfig::default())
        .expect("Fix: default dispatch_borrowed must delegate to dispatch");
    assert!(outputs.is_empty());
}
