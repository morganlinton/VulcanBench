//! Megakernel innovation / performance / security contracts.
//!
//! Aggressive external tests asserting desired invariants.  These tests
//! document the contract and are intended to fail if the implementation
//! drifts or regresses.

#![cfg(feature = "megakernel-batch")]
#![allow(deprecated)]
use std::time::Duration;
use vyre_driver_wgpu::megakernel::{BatchDispatchConfig, BatchDispatcher, BatchFile, FileBatch};
use vyre_runtime::megakernel::{
    control,
    io::{
        encode_empty_io_queue, try_complete_io_request, try_encode_empty_io_queue,
        try_poll_io_requests,
    },
    protocol,
    scheduler::{self, PRIORITY_STARVATION_COUNTER},
    BatchRuleProgram, Megakernel, MegakernelExecutionMode, MegakernelLaunchPolicy,
    MegakernelLaunchRequest, IO_SLOT_COUNT, IO_SLOT_WORDS,
};
use vyre_runtime::PipelineError;

const _: () = assert!(PRIORITY_STARVATION_COUNTER < protocol::CONTROL_MIN_WORDS);
const _: () = assert!(PRIORITY_STARVATION_COUNTER < control::OBSERVABLE_BASE);

// ---------------------------------------------------------------------------
// 1. No fixed sleeps in dispatch waits
// ---------------------------------------------------------------------------

mod megakernel_innovation_contracts_part1 {

    include!("__split/megakernel_innovation_contracts_part1.rs");
}
mod megakernel_innovation_contracts_part2 {
    include!("__split/megakernel_innovation_contracts_part2.rs");
}
