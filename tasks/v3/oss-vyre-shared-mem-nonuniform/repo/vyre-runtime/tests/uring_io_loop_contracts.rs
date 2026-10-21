//! Contracts for the autonomous io_uring megakernel IO loop.

#[test]
fn io_loop_does_not_use_fixed_thread_sleep() {
    let source = include_str!("../src/uring/io_loop.rs");
    assert!(
        !source.contains("thread::sleep"),
        "MegakernelIoLoop must not use fixed sleeps in the IO pump hot loop"
    );
    assert!(
        source.contains("park_timeout") || source.contains(".enter(0, 1, 1)"),
        "MegakernelIoLoop must wait through adaptive parking or io_uring completion waits"
    );
}

#[test]
fn io_loop_has_registered_buffer_fast_path() {
    let source = include_str!("../src/uring/io_loop.rs");
    assert!(
        source.contains("RegisteredIoDestination"),
        "MegakernelIoLoop must expose a registered destination table"
    );
    assert!(
        source.contains("submit_read_fixed_at"),
        "MegakernelIoLoop must route registered destinations through READ_FIXED"
    );
    assert!(
        !source.contains("submit_read_to_gpu_at_with_user_data"),
        "MegakernelIoLoop must not retain a compatibility READV route for unregistered GPU handles"
    );
    assert!(
        source.contains("unregistered GPU destination handle"),
        "MegakernelIoLoop must surface unregistered GPU handles as host-visible errors"
    );
}
