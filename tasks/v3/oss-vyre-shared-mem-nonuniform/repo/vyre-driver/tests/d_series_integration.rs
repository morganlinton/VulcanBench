//! Integration test wiring the D-series decision policies (D1, D2,
//! D3, D4, D9) and I2 trace-JIT speculation into a single end-to-end
//! decision flow. Proves the substrates compose: a runtime can call
//! all six in sequence on the same workload metrics and get coherent
//! verdicts.

use vyre_driver::arm_independence::{
    can_dispatch_concurrently, ArmBindingSummary, ArmIndependenceVerdict,
};
use vyre_driver::async_copy_overlap::{can_overlap_copy_with_kernel, CopyOverlapDecision};
use vyre_driver::bindless_policy::{
    decide_bindless, BindlessDecision, BindlessInputs, BindlessSupport,
};
use vyre_driver::command_reuse_policy::{
    decide_command_reuse, CommandReuseDecision, CommandReuseInputs,
};
use vyre_driver::dispatch_policy::{
    evaluate_dispatch_policy, DispatchExecutionMode, DispatchPolicyInputs,
};
use vyre_driver::persistent_kernel_policy::{
    decide_persistent_kernel, PersistentKernelDecision, PersistentKernelInputs,
};
use vyre_driver::trace_jit_policy::{
    decide_trace_jit_speculation, TraceJitDecision, TraceJitInputs,
};

fn arm_summary(reads: &[u32], writes: &[u32]) -> ArmBindingSummary {
    ArmBindingSummary {
        reads: reads.iter().copied().collect(),
        writes: writes.iter().copied().collect(),
    }
}

#[test]
fn high_throughput_workload_picks_aggressive_concurrency() {
    // Workload shape: large batch of small kernels with many bindings,
    // adjacent arms read/write disjoint slots, repeated dispatch with
    // identical pipeline. The substrate stack should pick:
    //   D1 PersistentKernel    (large batch + per-launch overhead)
    //   D2 Independent          (disjoint reads/writes)
    //   D3 Overlap              (copy targets a slot kernel doesn't read)
    //   D4 RecordAndReplay      (repeated dispatch shape)
    //   D9 Bindless             (40 resources >= threshold 24)
    //   I2 Speculate            (hot shape + high confidence + savings)

    // D1
    let pk = decide_persistent_kernel(PersistentKernelInputs {
        batch_size: 500,
        per_launch_overhead_ns: 5_000,
        per_item_kernel_ns: 1_000,
        persistent_setup_overhead_ns: 50_000,
    });
    assert!(matches!(
        pk,
        PersistentKernelDecision::PersistentKernel { .. }
    ));

    // D2  -  arms a (reads 0,1; writes 2) and b (reads 3,4; writes 5)
    let arm_a = arm_summary(&[0, 1], &[2]);
    let arm_b = arm_summary(&[3, 4], &[5]);
    assert_eq!(
        can_dispatch_concurrently(&arm_a, &arm_b),
        ArmIndependenceVerdict::Independent
    );

    // D3  -  copy targets slot 7 which arm_a doesn't read or write
    assert_eq!(
        can_overlap_copy_with_kernel(7, &arm_a),
        CopyOverlapDecision::Overlap
    );

    // D4
    let cg = decide_command_reuse(CommandReuseInputs {
        repeat_count: 500,
        per_launch_overhead_ns: 5_000,
        record_overhead_ns: 25_000,
        replay_overhead_ns: 500,
    });
    assert!(matches!(cg, CommandReuseDecision::RecordAndReplay { .. }));

    // D9
    assert_eq!(
        decide_bindless(BindlessInputs {
            resource_count: 40,
            support: BindlessSupport::Full,
            dynamic_indexing: true,
        }),
        BindlessDecision::Bindless
    );

    // I2
    let tj = decide_trace_jit_speculation(TraceJitInputs {
        shader_hit_count: 100,
        prediction_confidence_bps: 9_000,
        speculative_spec_cost_ns: 10_000,
        miss_cost_ns: 100_000,
    });
    assert!(matches!(tj, TraceJitDecision::Speculate { .. }));

    let bundled = evaluate_dispatch_policy(&DispatchPolicyInputs {
        persistent: PersistentKernelInputs {
            batch_size: 500,
            per_launch_overhead_ns: 5_000,
            per_item_kernel_ns: 1_000,
            persistent_setup_overhead_ns: 50_000,
        },
        arm_a,
        arm_b,
        copy_dst_slot: Some(7),
        graph: CommandReuseInputs {
            repeat_count: 500,
            per_launch_overhead_ns: 5_000,
            record_overhead_ns: 25_000,
            replay_overhead_ns: 500,
        },
        bindless: BindlessInputs {
            resource_count: 40,
            support: BindlessSupport::Full,
            dynamic_indexing: true,
        },
        trace_jit: TraceJitInputs {
            shader_hit_count: 100,
            prediction_confidence_bps: 9_000,
            speculative_spec_cost_ns: 10_000,
            miss_cost_ns: 100_000,
        },
    });
    assert_eq!(
        bundled.primary_execution_mode(),
        DispatchExecutionMode::PersistentKernel {
            savings_ns: 2_450_000
        },
        "bundled D-series policy must expose one primary launch strategy instead of leaving D1 and D4 conflict resolution to callers"
    );
}

#[test]
fn cold_low_confidence_workload_picks_conservative_paths() {
    // Workload shape: single dispatch, conflicting arms, copy onto a
    // read slot, no repetition, few resources, cold shape.
    //   D1 Standard, D2 Serialize, D3 Serialize, D4 Plain,
    //   D9 Traditional, I2 HoldSteady.

    let pk = decide_persistent_kernel(PersistentKernelInputs {
        batch_size: 1,
        per_launch_overhead_ns: 5_000,
        per_item_kernel_ns: 1_000,
        persistent_setup_overhead_ns: 50_000,
    });
    assert_eq!(pk, PersistentKernelDecision::StandardLaunches);

    let arm_a = arm_summary(&[5], &[1]);
    let arm_b = arm_summary(&[0], &[5]);
    assert!(matches!(
        can_dispatch_concurrently(&arm_a, &arm_b),
        ArmIndependenceVerdict::SerializeRequired { .. }
    ));

    assert_eq!(
        can_overlap_copy_with_kernel(5, &arm_a),
        CopyOverlapDecision::Serialize
    );

    let cg = decide_command_reuse(CommandReuseInputs {
        repeat_count: 1,
        per_launch_overhead_ns: 5_000,
        record_overhead_ns: 25_000,
        replay_overhead_ns: 500,
    });
    assert_eq!(cg, CommandReuseDecision::PlainLaunches);

    assert_eq!(
        decide_bindless(BindlessInputs {
            resource_count: 4,
            support: BindlessSupport::Full,
            dynamic_indexing: false,
        }),
        BindlessDecision::TraditionalBindings
    );

    let tj = decide_trace_jit_speculation(TraceJitInputs {
        shader_hit_count: 2,
        prediction_confidence_bps: 9_000,
        speculative_spec_cost_ns: 10_000,
        miss_cost_ns: 100_000,
    });
    assert_eq!(tj, TraceJitDecision::HoldSteady);

    let bundled = evaluate_dispatch_policy(&DispatchPolicyInputs {
        persistent: PersistentKernelInputs {
            batch_size: 1,
            per_launch_overhead_ns: 5_000,
            per_item_kernel_ns: 1_000,
            persistent_setup_overhead_ns: 50_000,
        },
        arm_a,
        arm_b,
        copy_dst_slot: Some(5),
        graph: CommandReuseInputs {
            repeat_count: 1,
            per_launch_overhead_ns: 5_000,
            record_overhead_ns: 25_000,
            replay_overhead_ns: 500,
        },
        bindless: BindlessInputs {
            resource_count: 4,
            support: BindlessSupport::Full,
            dynamic_indexing: false,
        },
        trace_jit: TraceJitInputs {
            shader_hit_count: 2,
            prediction_confidence_bps: 9_000,
            speculative_spec_cost_ns: 10_000,
            miss_cost_ns: 100_000,
        },
    });
    assert_eq!(
        bundled.primary_execution_mode(),
        DispatchExecutionMode::PlainLaunches
    );
}

#[test]
fn substrates_compose_without_panic_on_extreme_inputs() {
    // Adversarial: u32::MAX everywhere should not panic. Verifies
    // every substrate uses saturating arithmetic.
    let _ = decide_persistent_kernel(PersistentKernelInputs {
        batch_size: u32::MAX,
        per_launch_overhead_ns: u64::MAX / 2,
        per_item_kernel_ns: u64::MAX / 2,
        persistent_setup_overhead_ns: u64::MAX / 4,
    });

    let _ = decide_command_reuse(CommandReuseInputs {
        repeat_count: u32::MAX,
        per_launch_overhead_ns: u64::MAX / 2,
        record_overhead_ns: 1,
        replay_overhead_ns: 1,
    });

    let _ = decide_trace_jit_speculation(TraceJitInputs {
        shader_hit_count: u32::MAX,
        prediction_confidence_bps: 10_000,
        speculative_spec_cost_ns: 1,
        miss_cost_ns: u64::MAX,
    });

    let _ = decide_bindless(BindlessInputs {
        resource_count: u32::MAX,
        support: BindlessSupport::Full,
        dynamic_indexing: true,
    });

    let big_arm = arm_summary(
        &(0..1000).collect::<Vec<u32>>(),
        &(1000..2000).collect::<Vec<u32>>(),
    );
    let other = arm_summary(
        &(2000..3000).collect::<Vec<u32>>(),
        &(3000..4000).collect::<Vec<u32>>(),
    );
    let _ = can_dispatch_concurrently(&big_arm, &other);
    let _ = can_overlap_copy_with_kernel(u32::MAX, &big_arm);
}
