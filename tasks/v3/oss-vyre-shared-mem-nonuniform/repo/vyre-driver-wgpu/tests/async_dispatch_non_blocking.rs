//! Async dispatch must not block on GPU completion before returning the
//! pending handle.
//!
//! Guarantees:
//! - `dispatch_async` returns a `PendingDispatch` handle immediately
//! - The handle readiness can be observed without blocking
//! - Back-to-back async dispatches submit without host serialization
//! - Returned WGPU pending handles are object-safe

mod common;

use common::{
    assert_dispatch_async_ready_state_observable_for_non_trivial_work,
    assert_dispatch_async_returns_before_gpu_completion,
    assert_multiple_concurrent_async_dispatches_do_not_serialize,
    assert_pending_dispatch_from_wgpu_is_object_safe,
};

#[test]
fn dispatch_async_returns_before_gpu_completion() {
    assert_dispatch_async_returns_before_gpu_completion();
}

#[test]
fn dispatch_async_ready_state_is_observable_for_non_trivial_work() {
    assert_dispatch_async_ready_state_observable_for_non_trivial_work();
}

#[test]
fn multiple_concurrent_async_dispatches_do_not_serialize() {
    assert_multiple_concurrent_async_dispatches_do_not_serialize();
}

#[test]
fn pending_dispatch_from_wgpu_is_object_safe() {
    assert_pending_dispatch_from_wgpu_is_object_safe();
}
