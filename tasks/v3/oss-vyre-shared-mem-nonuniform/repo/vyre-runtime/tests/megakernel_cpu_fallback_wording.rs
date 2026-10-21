//! No CPU fallback wording  -  runtime-level contract.
//!
//! Guarantees that error messages and source code never imply a CPU
//! fallback path exists.

use vyre_runtime::megakernel::{
    descriptor::{BatchDescriptor, BuiltinOpcode, SlotDescriptor, SlotOpcode},
    protocol, Megakernel, MegakernelIoQueue, IO_SLOT_COUNT,
};
use vyre_runtime::PipelineError;

fn assert_no_cpu_wording(err: &PipelineError) {
    let msg = err.to_string().to_lowercase();
    assert!(!msg.contains("cpu"), "error must never mention CPU: {msg}");
    assert!(
        !msg.contains("fallback"),
        "error must never mention fallback: {msg}"
    );
    assert!(
        !msg.contains("software"),
        "error must never imply software emulation: {msg}"
    );
}

#[test]
fn encoder_errors_never_suggest_cpu_fallback() {
    let too_many_slots = (u32::MAX / protocol::SLOT_WORDS) + 1;
    assert_no_cpu_wording(
        &Megakernel::try_encode_empty_ring(too_many_slots).expect_err("must fail"),
    );

    assert_no_cpu_wording(
        &Megakernel::try_encode_control(false, 1, u32::MAX).expect_err("must fail"),
    );

    assert_no_cpu_wording(
        &Megakernel::try_encode_empty_debug_log(u32::MAX).expect_err("must fail"),
    );
}

#[test]
fn queue_full_errors_never_suggest_cpu_fallback() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = Megakernel::publish_slot(
        &mut ring,
        0,
        0,
        protocol::opcode::NOP,
        &[0u32; (protocol::ARGS_PER_SLOT + 1) as usize],
    )
    .expect_err("too many args must fail");
    assert_no_cpu_wording(&err);

    let err = MegakernelIoQueue::new(0).expect_err("zero slots must fail");
    assert_no_cpu_wording(&err);

    let err = MegakernelIoQueue::new(IO_SLOT_COUNT + 1).expect_err("oversized must fail");
    assert_no_cpu_wording(&err);
}

#[test]
fn batch_descriptor_errors_never_suggest_cpu_fallback() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let batch = BatchDescriptor::new(
        0,
        vec![
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
        ],
    );
    let err = batch
        .publish_into(&mut ring)
        .expect_err("overflow must fail");
    assert_no_cpu_wording(&err);
}

#[test]
fn runtime_megakernel_source_contains_no_cpu_fallback_path() {
    let paths = ["src/megakernel/mod.rs"];
    for path in paths {
        let src = std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|_| panic!("{path} must be readable"));
        let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);
        let lower = prod.to_lowercase();
        assert!(
            !lower.contains("cpu fallback"),
            "{path} must not contain 'cpu fallback'"
        );
        assert!(
            !lower.contains("software fallback"),
            "{path} must not contain 'software fallback'"
        );
    }
}
