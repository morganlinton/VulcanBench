//! D4 substrate: pre-recorded command reuse policy.
//!
//! When the same dispatch shape repeats (same Program, same binding
//! handles, same workgroup, same workload count), backends can record
//! the launch sequence once and replay it through their native command
//! reuse primitive. This eliminates per-launch driver API overhead.
//!
//! Pure decision: given a dispatch repetition count and the measured
//! per-launch overhead vs command-record overhead, should the
//! dispatcher record-and-replay or just launch normally?
//!
//! This sits next to D1 (persistent kernels). Persistent mode wins
//! for unpredictable batches of small kernels; command reuse wins for
//! REPEATED dispatches of the same shape.

/// Inputs to the command-reuse decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandReuseInputs {
    /// Number of times this exact dispatch shape will be repeated
    /// (the same Program + bindings + workload count).
    pub repeat_count: u32,
    /// Per-launch driver API overhead in nanoseconds. Same number
    /// the persistent-kernel policy uses.
    pub per_launch_overhead_ns: u64,
    /// One-time cost of recording the native command sequence.
    pub record_overhead_ns: u64,
    /// Per-replay cost of the native command-reuse primitive.
    pub replay_overhead_ns: u64,
}

/// Verdict from [`decide_command_reuse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandReuseDecision {
    /// Use plain dispatch  -  repeat count too low to amortise the
    /// command-record cost.
    PlainLaunches,
    /// Record once, replay `repeat_count - 1` more times. Includes
    /// the predicted savings vs plain launches for telemetry.
    RecordAndReplay {
        /// Predicted total time saved (in nanoseconds) vs plain
        /// launches. Positive by construction.
        savings_ns: u128,
    },
}

/// Decide whether to record a command sequence once and replay it for
/// the remaining `repeat_count - 1` dispatches.
///
/// Plain cost:    `repeat * per_launch_ovh`
/// Reuse cost:    `record_ovh + repeat * replay_ovh`
/// Reuse wins iff `repeat * (per_launch_ovh - replay_ovh) > record_ovh`.
#[must_use]
pub fn decide_command_reuse(inputs: CommandReuseInputs) -> CommandReuseDecision {
    if inputs.repeat_count <= 1 {
        return CommandReuseDecision::PlainLaunches;
    }
    if inputs.per_launch_overhead_ns <= inputs.replay_overhead_ns {
        // Replay is not actually cheaper than plain launch.
        // recording costs us bytes for nothing.
        return CommandReuseDecision::PlainLaunches;
    }
    let per_call_savings =
        u128::from(inputs.per_launch_overhead_ns) - u128::from(inputs.replay_overhead_ns);
    let total_call_savings = u128::from(inputs.repeat_count) * per_call_savings;
    let record_overhead_ns = u128::from(inputs.record_overhead_ns);
    if total_call_savings <= record_overhead_ns {
        return CommandReuseDecision::PlainLaunches;
    }
    let savings_ns = total_call_savings - record_overhead_ns;
    CommandReuseDecision::RecordAndReplay { savings_ns }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(rep: u32, launch: u64, record: u64, replay: u64) -> CommandReuseInputs {
        CommandReuseInputs {
            repeat_count: rep,
            per_launch_overhead_ns: launch,
            record_overhead_ns: record,
            replay_overhead_ns: replay,
        }
    }

    #[test]
    fn single_dispatch_is_plain() {
        // No repetition → recording wastes work.
        assert_eq!(
            decide_command_reuse(inp(1, 5_000, 25_000, 500)),
            CommandReuseDecision::PlainLaunches
        );
    }

    #[test]
    fn zero_repeat_is_plain() {
        assert_eq!(
            decide_command_reuse(inp(0, 5_000, 25_000, 500)),
            CommandReuseDecision::PlainLaunches
        );
    }

    #[test]
    fn replay_no_cheaper_than_launch_is_plain() {
        // Graph replay = per-launch overhead → no savings possible.
        assert_eq!(
            decide_command_reuse(inp(1000, 5_000, 25_000, 5_000)),
            CommandReuseDecision::PlainLaunches
        );
    }

    #[test]
    fn small_repeat_under_amortisation_is_plain() {
        // 5 repeats × (5000 - 500) savings = 22_500; record costs 25_000.
        assert_eq!(
            decide_command_reuse(inp(5, 5_000, 25_000, 500)),
            CommandReuseDecision::PlainLaunches
        );
    }

    #[test]
    fn large_repeat_above_amortisation_picks_record_and_replay() {
        // 100 repeats × 4_500 savings = 450_000; record 25_000.
        // Net savings = 425_000.
        assert_eq!(
            decide_command_reuse(inp(100, 5_000, 25_000, 500)),
            CommandReuseDecision::RecordAndReplay {
                savings_ns: 425_000
            }
        );
    }

    #[test]
    fn savings_strictly_positive_when_record_and_replay() {
        let dec = decide_command_reuse(inp(1000, 5_000, 25_000, 500));
        match dec {
            CommandReuseDecision::RecordAndReplay { savings_ns } => assert!(savings_ns > 0),
            other => panic!("expected RecordAndReplay; got {:?}", other),
        }
    }

    #[test]
    fn widened_arithmetic_preserves_extreme_savings() {
        // u32::MAX repeats × u64-near-max savings shouldn't panic.
        let dec = decide_command_reuse(inp(u32::MAX, u64::MAX / 2, 25_000, 1));
        match dec {
            CommandReuseDecision::RecordAndReplay { savings_ns } => {
                assert_eq!(
                    savings_ns,
                    u128::from(u32::MAX) * (u128::from(u64::MAX / 2) - 1) - 25_000
                );
            }
            other => panic!("expected RecordAndReplay; got {:?}", other),
        }
    }

    #[test]
    fn command_reuse_policy_source_uses_exact_widened_arithmetic() {
        let source = include_str!("command_reuse_policy.rs");

        assert!(
            !source.contains(concat!("saturating", "_mul"))
                && !source.contains(concat!("saturating", "_sub")),
            "Fix: command-reuse policy must use exact widened arithmetic, not saturating replay-cost math."
        );
        assert!(
            source.contains("u128::from(inputs.per_launch_overhead_ns)")
                && source.contains("u128::from(inputs.repeat_count)")
                && source.contains("total_call_savings - record_overhead_ns"),
            "Fix: command-reuse savings must stay widened through the verdict."
        );
    }
}
