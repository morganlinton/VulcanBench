//! Failure-oriented contract tests for async dispatch.
//!
//! Guarantees:
//! - `dispatch_async` is always async (returns PendingDispatch immediately)
//! - Errors from the synchronous path propagate immediately, not through the handle
//! - The default async adapter never blocks the caller thread
//! - `PendingDispatch` remains object-safe

use std::sync::atomic::{AtomicUsize, Ordering};

use vyre_driver::{BackendError, DispatchConfig, PendingDispatch, VyreBackend};
use vyre_foundation::ir::Program;

struct FailingBackend;

impl vyre_driver::backend::private::Sealed for FailingBackend {}

impl VyreBackend for FailingBackend {
    fn id(&self) -> &'static str {
        "failing"
    }

    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        Err(BackendError::new(
            "injected failure. Fix: this is a test fixture.",
        ))
    }
}

struct CountingBackend {
    dispatch_calls: AtomicUsize,
}

impl vyre_driver::backend::private::Sealed for CountingBackend {}

impl VyreBackend for CountingBackend {
    fn id(&self) -> &'static str {
        "counting"
    }

    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        self.dispatch_calls.fetch_add(1, Ordering::Relaxed);
        Ok(inputs.to_vec())
    }
}

#[test]
fn dispatch_async_propagates_error_immediately() {
    let backend = FailingBackend;
    let program = Program::default();
    let result = backend.dispatch_async(&program, &[vec![1, 2, 3]], &DispatchConfig::default());
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("dispatch_async must return Err immediately when dispatch fails"),
    };
    assert!(
        format!("{err}").contains("injected failure"),
        "error must surface the underlying failure message; got: {err}"
    );
}

#[test]
fn default_dispatch_async_never_blocks_and_returns_ready_handle() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let pending = backend
        .dispatch_async(&program, &[vec![1, 2, 3]], &DispatchConfig::default())
        .expect("default dispatch_async must succeed");
    assert!(
        pending.is_ready(),
        "default dispatch_async must return a ready handle so callers never spin-wait"
    );
    let outputs = pending
        .await_result()
        .expect("default dispatch_async result must be retrievable");
    assert_eq!(outputs, vec![vec![1, 2, 3]]);
    assert_eq!(
        backend.dispatch_calls.load(Ordering::Relaxed),
        1,
        "default dispatch_async must call dispatch exactly once"
    );
}

#[test]
fn pending_dispatch_trait_is_object_safe() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let pending: Box<dyn PendingDispatch> = backend
        .dispatch_async(&program, &[vec![4, 5, 6]], &DispatchConfig::default())
        .expect("dispatch_async must produce object-safe PendingDispatch");
    assert!(pending.is_ready());
    let outputs = pending
        .await_result()
        .expect("object-safe await must succeed");
    assert_eq!(outputs, vec![vec![4, 5, 6]]);
}

#[test]
fn dispatch_borrowed_async_preserves_semantics() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let inputs: Vec<Vec<u8>> = vec![vec![7, 8, 9]];
    let borrowed: Vec<&[u8]> = inputs.iter().map(|v| v.as_slice()).collect();
    let outputs = backend
        .dispatch_borrowed(&program, &borrowed, &DispatchConfig::default())
        .expect("dispatch_borrowed must forward to dispatch by default");
    assert_eq!(outputs, vec![vec![7, 8, 9]]);
}
