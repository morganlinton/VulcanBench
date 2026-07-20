//! Failure-oriented tests for backend capability negotiation.
//!
//! Guarantees:
//! - Unknown backends report conservative defaults (no dispatch, lowest precedence)
//! - The `Backend` blanket impl for `VyreBackend` never drifts
//! - Capability traits are object-safe so routers can hold `dyn` pointers

use vyre_driver::backend::{
    acquire_preferred_dispatch_backend, backend_dispatches, backend_precedence,
    registered_backends, registered_backends_by_precedence, Backend, BackendError, DispatchConfig,
    Executable, Streamable, VyreBackend,
};
use vyre_foundation::ir::Program;

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

#[test]
fn unknown_backend_reports_no_dispatch_capability() {
    assert!(
        !backend_dispatches("nonexistent-backend"),
        "Fix: unknown backends must report dispatches=false"
    );
}

#[test]
fn unknown_backend_gets_lowest_precedence() {
    assert_eq!(
        backend_precedence("nonexistent-backend"),
        u32::MAX,
        "Fix: unknown backends must get lowest precedence"
    );
}

#[test]
fn empty_registry_by_precedence_is_empty() {
    // vyre-driver has no backend dependencies, so the in-crate view is empty
    let sorted = registered_backends_by_precedence();
    assert!(
        sorted.is_empty(),
        "Fix: vyre-driver alone must see zero backends; sorted list was non-empty"
    );
}

#[test]
fn empty_registry_iter_is_empty() {
    assert!(
        registered_backends().is_empty(),
        "Fix: vyre-driver alone must see zero backends"
    );
}

#[test]
fn preferred_dispatch_backend_fails_loudly_without_cpu_fallback() {
    let err = match acquire_preferred_dispatch_backend() {
        Ok(backend) => panic!(
            "vyre-driver alone must not acquire backend `{}` as preferred dispatch",
            backend.id()
        ),
        Err(err) => err,
    };
    let msg = err.to_string();
    let lower = msg.to_lowercase();

    assert!(
        msg.contains("no usable GPU dispatch backend is available"),
        "Fix: preferred dispatch acquisition must name the missing GPU dispatch backend: {msg}"
    );
    assert!(
        msg.contains("repair the GPU driver probe"),
        "Fix: acquisition failure must tell operators to repair the GPU probe: {msg}"
    );
    assert!(
        !lower.contains("falling back") && !lower.contains("fallback"),
        "Fix: preferred dispatch acquisition must never advertise fallback: {msg}"
    );
}

#[test]
fn backend_trait_blanket_impl_for_vyre_backend() {
    let backend = MinimalBackend;
    // These compile only if the blanket impl is alive.
    let as_backend: &dyn Backend = &backend;
    assert_eq!(as_backend.id(), "minimal");
    assert_eq!(as_backend.version(), "unspecified");
    // default_supported_ops returns the core op set; we only check it doesn't panic
    let _ = as_backend.supported_ops();
}

#[test]
fn backend_trait_is_object_safe() {
    // Compilation guard  -  if Backend ever loses object safety this test stops compiling.
    let _: Option<Box<dyn Backend>> = None;
}

#[test]
fn executable_trait_is_object_safe() {
    // Compilation guard  -  if Executable ever loses object safety this test stops compiling.
    let _: Option<Box<dyn Executable>> = None;
}

#[test]
fn streamable_trait_is_object_safe() {
    // Compilation guard  -  if Streamable ever loses object safety this test stops compiling.
    let _: Option<Box<dyn Streamable>> = None;
}
