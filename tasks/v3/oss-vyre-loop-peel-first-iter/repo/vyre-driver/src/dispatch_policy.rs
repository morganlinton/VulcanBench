//! Bundled D-series + I2 policy invocation.
//!
//! The runtime dispatcher needs all six decisions for every batch
//! (D1 persistent kernel, D2 arm independence, D3 async copy
//! overlap, D4 command reuse, D9 bindless, I2 trace-JIT
//! speculation). Calling six functions and threading six verdicts
//! through the dispatcher is boilerplate. This module owns the
//! one-shot bundle: pass `DispatchPolicyInputs`, get back a
//! `DispatchPolicyVerdict` with every sub-decision already made.
//!
//! Pure composition  -  no new logic, just sequential calls into the
//! per-substrate decide_* functions. Each verdict carries the
//! sub-substrate's typed result so callers can match exhaustively.

use crate::arm_independence::{
    can_dispatch_concurrently, ArmBindingSummary, ArmIndependenceVerdict,
};
use crate::async_copy_overlap::{can_overlap_copy_with_kernel, CopyOverlapDecision};
use crate::bindless_policy::{decide_bindless, BindlessDecision, BindlessInputs};
use crate::command_reuse_policy::{decide_command_reuse, CommandReuseDecision, CommandReuseInputs};
use crate::observability::{record_substrate_audit_event, SubstrateAuditEvent};
use crate::persistent_kernel_policy::{
    decide_persistent_kernel, PersistentKernelDecision, PersistentKernelInputs,
};
use crate::trace_jit_policy::{decide_trace_jit_speculation, TraceJitDecision, TraceJitInputs};

/// Input bundle for a single dispatch-policy invocation.
///
/// Two arms (`arm_a`, `arm_b`) are needed for D2 / D3 even when
/// only one is real  -  pass an empty `ArmBindingSummary::default()`
/// for the absent slot. The `copy_dst_slot` is `None` when no H2D
/// copy is queued for this batch.
#[derive(Debug, Clone)]
pub struct DispatchPolicyInputs {
    /// D1 persistent-kernel inputs.
    pub persistent: PersistentKernelInputs,
    /// First arm of the D2 pair (also the kernel side of the D3 copy).
    pub arm_a: ArmBindingSummary,
    /// Second arm of the D2 pair.
    pub arm_b: ArmBindingSummary,
    /// D3 copy destination slot, or `None` when no H2D copy is queued.
    pub copy_dst_slot: Option<u32>,
    /// D4 command-reuse inputs.
    pub graph: CommandReuseInputs,
    /// D9 bindless inputs.
    pub bindless: BindlessInputs,
    /// I2 trace-JIT speculation inputs.
    pub trace_jit: TraceJitInputs,
}

/// Result bundle from a single dispatch-policy invocation. Every
/// sub-substrate verdict appears in its typed form.
#[derive(Debug, Clone)]
pub struct DispatchPolicyVerdict {
    /// D1 persistent-kernel verdict.
    pub persistent: PersistentKernelDecision,
    /// D2 arm-independence verdict for the (arm_a, arm_b) pair.
    pub arm_independence: ArmIndependenceVerdict,
    /// `None` when the inputs had no `copy_dst_slot`; otherwise
    /// the D3 substrate's verdict for that copy.
    pub copy_overlap: Option<CopyOverlapDecision>,
    /// D4 command-reuse verdict.
    pub command_reuse: CommandReuseDecision,
    /// D9 bindless verdict.
    pub bindless: BindlessDecision,
    /// I2 trace-JIT speculation verdict.
    pub trace_jit: TraceJitDecision,
}

/// Mutually exclusive launch strategy selected from the dispatch-policy bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchExecutionMode {
    /// Plain launches remain cheapest for this batch.
    PlainLaunches,
    /// Use persistent kernel mode.
    PersistentKernel {
        /// Predicted saved nanoseconds versus plain launches.
        savings_ns: u128,
    },
    /// Use native command record/replay.
    CommandReuse {
        /// Predicted saved nanoseconds versus plain launches.
        savings_ns: u128,
    },
}

impl DispatchPolicyVerdict {
    /// Return the mutually exclusive primary launch strategy.
    ///
    /// D1 persistent kernels and D4 command reuse can both be profitable on
    /// paper. A concrete dispatcher cannot run both for the same launch group,
    /// so this resolver chooses the higher predicted savings. Equal savings
    /// prefer command reuse because it avoids persistent queue residency.
    #[must_use]
    pub fn primary_execution_mode(&self) -> DispatchExecutionMode {
        select_primary_execution_mode(self.persistent, self.command_reuse)
    }
}

/// One-shot evaluation of every dispatch-side policy substrate.
#[must_use]
pub fn evaluate_dispatch_policy(inputs: &DispatchPolicyInputs) -> DispatchPolicyVerdict {
    let persistent = decide_persistent_kernel(inputs.persistent);
    let arm_independence = can_dispatch_concurrently(&inputs.arm_a, &inputs.arm_b);
    let copy_overlap = inputs
        .copy_dst_slot
        .map(|slot| can_overlap_copy_with_kernel(slot, &inputs.arm_a));
    let command_reuse = decide_command_reuse(inputs.graph);
    let bindless = decide_bindless(inputs.bindless);
    let trace_jit = decide_trace_jit_speculation(inputs.trace_jit);
    record_policy_audit_events(persistent, command_reuse, bindless, trace_jit);
    DispatchPolicyVerdict {
        persistent,
        arm_independence,
        copy_overlap,
        command_reuse,
        bindless,
        trace_jit,
    }
}

/// Select a single primary launch strategy from D1 and D4 decisions.
#[must_use]
pub fn select_primary_execution_mode(
    persistent: PersistentKernelDecision,
    command_reuse: CommandReuseDecision,
) -> DispatchExecutionMode {
    match (persistent, command_reuse) {
        (
            PersistentKernelDecision::PersistentKernel {
                savings_ns: persistent_savings,
            },
            CommandReuseDecision::RecordAndReplay {
                savings_ns: command_savings,
            },
        ) => {
            if persistent_savings > command_savings {
                DispatchExecutionMode::PersistentKernel {
                    savings_ns: persistent_savings,
                }
            } else {
                DispatchExecutionMode::CommandReuse {
                    savings_ns: command_savings,
                }
            }
        }
        (
            PersistentKernelDecision::PersistentKernel { savings_ns },
            CommandReuseDecision::PlainLaunches,
        ) => DispatchExecutionMode::PersistentKernel { savings_ns },
        (
            PersistentKernelDecision::StandardLaunches,
            CommandReuseDecision::RecordAndReplay { savings_ns },
        ) => DispatchExecutionMode::CommandReuse { savings_ns },
        (PersistentKernelDecision::StandardLaunches, CommandReuseDecision::PlainLaunches) => {
            DispatchExecutionMode::PlainLaunches
        }
    }
}

fn record_policy_audit_events(
    persistent: PersistentKernelDecision,
    command_reuse: CommandReuseDecision,
    bindless: BindlessDecision,
    trace_jit: TraceJitDecision,
) {
    record_policy_audit_events_with(
        persistent,
        command_reuse,
        bindless,
        trace_jit,
        record_substrate_audit_event,
    );
}

fn record_policy_audit_events_with(
    persistent: PersistentKernelDecision,
    command_reuse: CommandReuseDecision,
    bindless: BindlessDecision,
    trace_jit: TraceJitDecision,
    mut record: impl FnMut(SubstrateAuditEvent),
) {
    if let PersistentKernelDecision::PersistentKernel { savings_ns } = persistent {
        record(SubstrateAuditEvent {
            substrate: "persistent_kernel",
            action: "queue_batch",
            saved_ns: savings_ns,
            detail: "launch_overhead",
        });
    }
    if let CommandReuseDecision::RecordAndReplay { savings_ns } = command_reuse {
        record(SubstrateAuditEvent {
            substrate: "command_reuse",
            action: "record_and_replay",
            saved_ns: savings_ns,
            detail: "repeat_shape",
        });
    }
    if bindless == BindlessDecision::Bindless {
        record(SubstrateAuditEvent {
            substrate: "bindless",
            action: "descriptor_array",
            saved_ns: 0,
            detail: "resource_count_threshold",
        });
    }
    if let TraceJitDecision::Speculate {
        expected_savings_ns,
    } = trace_jit
    {
        record(SubstrateAuditEvent {
            substrate: "trace_jit",
            action: "speculate",
            saved_ns: expected_savings_ns,
            detail: "predicted_shape",
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindless_policy::BindlessSupport;

    fn arm(reads: &[u32], writes: &[u32]) -> ArmBindingSummary {
        ArmBindingSummary {
            reads: reads.iter().copied().collect(),
            writes: writes.iter().copied().collect(),
        }
    }

    fn aggressive_inputs() -> DispatchPolicyInputs {
        DispatchPolicyInputs {
            persistent: PersistentKernelInputs {
                batch_size: 500,
                per_launch_overhead_ns: 5_000,
                per_item_kernel_ns: 1_000,
                persistent_setup_overhead_ns: 50_000,
            },
            arm_a: arm(&[0, 1], &[2]),
            arm_b: arm(&[3, 4], &[5]),
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
        }
    }

    fn conservative_inputs() -> DispatchPolicyInputs {
        DispatchPolicyInputs {
            persistent: PersistentKernelInputs {
                batch_size: 1,
                per_launch_overhead_ns: 5_000,
                per_item_kernel_ns: 1_000,
                persistent_setup_overhead_ns: 50_000,
            },
            arm_a: arm(&[5], &[1]),
            arm_b: arm(&[0], &[5]),
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
        }
    }

    #[test]
    fn aggressive_workload_routes_through_every_aggressive_path() {
        let _guard = crate::observability::audit_events_test_lock();
        crate::observability::clear_substrate_audit_events_for_test();
        let v = evaluate_dispatch_policy(&aggressive_inputs());
        assert!(matches!(
            v.persistent,
            PersistentKernelDecision::PersistentKernel { .. }
        ));
        assert_eq!(v.arm_independence, ArmIndependenceVerdict::Independent);
        assert_eq!(v.copy_overlap, Some(CopyOverlapDecision::Overlap));
        assert!(matches!(
            v.command_reuse,
            CommandReuseDecision::RecordAndReplay { .. }
        ));
        assert_eq!(v.bindless, BindlessDecision::Bindless);
        assert!(matches!(v.trace_jit, TraceJitDecision::Speculate { .. }));
        assert_eq!(
            v.primary_execution_mode(),
            DispatchExecutionMode::PersistentKernel {
                savings_ns: 2_450_000
            }
        );
        record_policy_audit_events_with(
            v.persistent,
            v.command_reuse,
            v.bindless,
            v.trace_jit,
            crate::observability::record_substrate_audit_event_for_test,
        );
        let log = crate::observability::snapshot_for_test().to_audit_log();
        assert!(log.contains("persistent_kernel queue_batch"));
        assert!(log.contains("command_reuse record_and_replay"));
        assert!(log.contains("bindless descriptor_array"));
        assert!(log.contains("trace_jit speculate"));
        crate::observability::clear_substrate_audit_events_for_test();
    }

    #[test]
    fn conservative_workload_routes_through_every_conservative_path() {
        let v = evaluate_dispatch_policy(&conservative_inputs());
        assert_eq!(v.persistent, PersistentKernelDecision::StandardLaunches);
        assert!(matches!(
            v.arm_independence,
            ArmIndependenceVerdict::SerializeRequired { .. }
        ));
        assert_eq!(v.copy_overlap, Some(CopyOverlapDecision::Serialize));
        assert_eq!(v.command_reuse, CommandReuseDecision::PlainLaunches);
        assert_eq!(v.bindless, BindlessDecision::TraditionalBindings);
        assert_eq!(v.trace_jit, TraceJitDecision::HoldSteady);
        assert_eq!(
            v.primary_execution_mode(),
            DispatchExecutionMode::PlainLaunches
        );
    }

    #[test]
    fn missing_copy_slot_reports_none_for_overlap() {
        // When the dispatcher has no H2D copy queued, copy_overlap
        // should return None instead of fabricating a verdict.
        let mut inputs = aggressive_inputs();
        inputs.copy_dst_slot = None;
        let v = evaluate_dispatch_policy(&inputs);
        assert_eq!(v.copy_overlap, None);
    }

    #[test]
    fn primary_execution_mode_prefers_command_reuse_on_equal_savings() {
        let mode = select_primary_execution_mode(
            PersistentKernelDecision::PersistentKernel { savings_ns: 100 },
            CommandReuseDecision::RecordAndReplay { savings_ns: 100 },
        );
        assert_eq!(
            mode,
            DispatchExecutionMode::CommandReuse { savings_ns: 100 }
        );
    }

    #[test]
    fn primary_execution_mode_selects_only_profitable_substrate() {
        assert_eq!(
            select_primary_execution_mode(
                PersistentKernelDecision::PersistentKernel { savings_ns: 500 },
                CommandReuseDecision::PlainLaunches,
            ),
            DispatchExecutionMode::PersistentKernel { savings_ns: 500 }
        );
        assert_eq!(
            select_primary_execution_mode(
                PersistentKernelDecision::StandardLaunches,
                CommandReuseDecision::RecordAndReplay { savings_ns: 700 },
            ),
            DispatchExecutionMode::CommandReuse { savings_ns: 700 }
        );
    }
}
