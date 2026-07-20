//! Async dispatch always non-blocking contract tests for the default `VyreBackend` impl.
//!
//! Guarantees:
//! - `dispatch_async` returns a `PendingDispatch` handle without blocking beyond the
//!   synchronous `dispatch` call that the default performs
//! - Errors from the underlying `dispatch` propagate immediately, not deferred
//! - Multiple `dispatch_async` calls produce independent handles
//! - `PendingDispatch` remains object-safe and consumable

use std::sync::atomic::{AtomicUsize, Ordering};
use vyre_driver::{BackendError, DispatchConfig, PendingDispatch, VyreBackend};
use vyre_foundation::ir::Program;

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

struct ErrorBackend;

impl vyre_driver::backend::private::Sealed for ErrorBackend {}

impl VyreBackend for ErrorBackend {
    fn id(&self) -> &'static str {
        "error"
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

// ------------------------------------------------------------------
// 1. dispatch_async never blocks the caller beyond the default dispatch
// ------------------------------------------------------------------

#[test]
fn dispatch_async_returns_handle_immediately() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let pending = backend
        .dispatch_async(&program, &[vec![1, 2, 3]], &DispatchConfig::default())
        .expect("Fix: dispatch_async must return a handle immediately");
    assert!(pending.is_ready());
    let outputs = pending
        .await_result()
        .expect("Fix: await_result must succeed");
    assert_eq!(outputs, vec![vec![1, 2, 3]]);
    assert_eq!(backend.dispatch_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn dispatch_async_produces_independent_handles() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let p1 = backend
        .dispatch_async(&program, &[vec![1]], &DispatchConfig::default())
        .unwrap();
    let p2 = backend
        .dispatch_async(&program, &[vec![2]], &DispatchConfig::default())
        .unwrap();
    let p3 = backend
        .dispatch_async(&program, &[vec![3]], &DispatchConfig::default())
        .unwrap();

    assert_eq!(p1.await_result().unwrap(), vec![vec![1]]);
    assert_eq!(p2.await_result().unwrap(), vec![vec![2]]);
    assert_eq!(p3.await_result().unwrap(), vec![vec![3]]);
    assert_eq!(backend.dispatch_calls.load(Ordering::Relaxed), 3);
}

// ------------------------------------------------------------------
// 2. Errors propagate immediately, never deferred
// ------------------------------------------------------------------

#[test]
fn dispatch_async_error_is_immediate_not_deferred() {
    let backend = ErrorBackend;
    let result = backend.dispatch_async(&Program::default(), &[], &DispatchConfig::default());
    assert!(
        result.is_err(),
        "Fix: dispatch_async must propagate dispatch errors immediately"
    );
    let err = match result {
        Ok(_) => unreachable!("checked above"),
        Err(error) => error,
    };
    assert!(
        err.to_string().contains("injected failure"),
        "Fix: error message must contain the underlying failure"
    );
}

// ------------------------------------------------------------------
// 3. PendingDispatch object safety and consumption
// ------------------------------------------------------------------

#[test]
fn pending_dispatch_is_object_safe_and_consumable() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let pending: Box<dyn PendingDispatch> = backend
        .dispatch_async(&program, &[vec![4, 5, 6]], &DispatchConfig::default())
        .expect("Fix: dispatch_async must produce object-safe PendingDispatch");
    assert!(pending.is_ready());
    let outputs = pending
        .await_result()
        .expect("Fix: object-safe await must succeed");
    assert_eq!(outputs, vec![vec![4, 5, 6]]);
}

#[test]
fn pending_dispatch_can_be_awaited_after_is_ready_true() {
    let backend = CountingBackend {
        dispatch_calls: AtomicUsize::new(0),
    };
    let program = Program::default();
    let pending = backend
        .dispatch_async(&program, &[vec![7, 8]], &DispatchConfig::default())
        .unwrap();
    // Poll until ready (default impl is immediately ready).
    assert!(pending.is_ready());
    let outputs = pending.await_result().unwrap();
    assert_eq!(outputs, vec![vec![7, 8]]);
}
