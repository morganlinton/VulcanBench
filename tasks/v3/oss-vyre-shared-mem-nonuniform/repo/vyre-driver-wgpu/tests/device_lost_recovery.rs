//! Device-loss recovery.
//!
//! See `contracts/release.md`. After a simulated device-lost event
//! the backend must (a) report `device_lost() == true`, (b) recover
//! via `try_recover() -> Ok(())`, (c) accept the next dispatch
//! successfully.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::DispatchConfig;
use vyre_driver::VyreBackend;
use vyre_driver_wgpu::engine::persistent::{
    run_persistent_kernel, PersistentPayloadWorkItem, PersistentQueue,
};
use vyre_driver_wgpu::WgpuBackend;

fn add_one_program(words: u32) -> Program {
    let idx = Expr::gid_x();
    let in_bounds = Expr::lt(idx.clone(), Expr::u32(words));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(words),
            BufferDecl::output("out", 1, DataType::U32)
                .with_count(words)
                .with_output_byte_range(0..(words as usize * 4)),
        ],
        [64, 1, 1],
        vec![
            Node::if_then(
                in_bounds,
                vec![Node::store(
                    "out",
                    idx.clone(),
                    Expr::add(Expr::load("input", idx), Expr::u32(1)),
                )],
            ),
            Node::return_(),
        ],
    )
}

#[test]
fn device_lost_recovery_round_trip() {
    let backend = WgpuBackend::acquire().expect("Fix: GPU must be available for recovery test");

    // Simulate device loss through the backend test hook. Recovery must
    // invalidate device-local caches and reacquire the same adapter identity.
    backend
        .force_device_lost()
        .expect("Fix: test hook must invalidate the cached device");

    assert!(
        backend.device_lost(),
        "after force_device_lost the probe must return true"
    );

    backend
        .try_recover()
        .expect("device-loss recovery: try_recover must succeed");

    assert!(
        !backend.device_lost(),
        "after try_recover the device_lost probe must return false"
    );

    // The backend must dispatch successfully after recovery.
    let program = vyre::Program::empty();
    let _ = backend
        .dispatch(&program, &[], &vyre::DispatchConfig::default())
        .expect("Fix: dispatch must succeed after device recovery");
}

#[test]
fn persistent_kernel_recovery_round_trip() {
    let backend =
        WgpuBackend::acquire().expect("Fix: GPU must be available for persistent recovery test");
    let program = add_one_program(1);
    let mut queue = PersistentQueue::new();
    queue.push(PersistentPayloadWorkItem {
        id: 0,
        payload: 42u32.to_le_bytes().to_vec(),
    });

    // First persistent dispatch must succeed.
    let report1 = run_persistent_kernel(
        &backend,
        &program,
        &DispatchConfig::default(),
        queue.clone(),
    )
    .expect("Fix: persistent dispatch must succeed before device loss");
    assert_eq!(report1.results.len(), 1);
    let actual1 = u32::from_le_bytes([
        report1.results[0].payload[0],
        report1.results[0].payload[1],
        report1.results[0].payload[2],
        report1.results[0].payload[3],
    ]);
    assert_eq!(actual1, 43);

    // Simulate device loss.
    backend
        .force_device_lost()
        .expect("Fix: test hook must invalidate the cached device");
    assert!(backend.device_lost());

    // Persistent dispatch must auto-recover and succeed.
    let report2 = run_persistent_kernel(&backend, &program, &DispatchConfig::default(), queue)
        .expect("Fix: persistent dispatch must succeed after device recovery");
    assert_eq!(report2.results.len(), 1);
    let actual2 = u32::from_le_bytes([
        report2.results[0].payload[0],
        report2.results[0].payload[1],
        report2.results[0].payload[2],
        report2.results[0].payload[3],
    ]);
    assert_eq!(actual2, 43);
}
