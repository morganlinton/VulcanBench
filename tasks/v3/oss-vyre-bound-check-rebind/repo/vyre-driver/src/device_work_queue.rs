#![allow(unused_imports)]
//! Backend-neutral device-side work queue planning for dependent dataflow execution.

use crate::numeric::BackendNumericPolicy;

const DEVICE_WORK_QUEUE_NUMERIC: BackendNumericPolicy =
    BackendNumericPolicy::new("device work queue");

/// Host synchronization policy for a device device-side work queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkQueueHostSync {
    /// Host reads only final completion state after device-side draining.
    FinalOnly,
    /// Host participates during queue draining.
    HostParticipates,
}

/// Work queue workload profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceWorkQueueProfile {
    /// Initial active work items enqueued before launch.
    pub initial_items: u64,
    /// Maximum resident queue capacity in work items.
    pub queue_capacity: u64,
    /// ABI bytes per queue entry.
    pub entry_bytes: u64,
    /// Bytes required for queue head/tail counters and changed flags.
    pub control_bytes: u64,
    /// Caller-approved device-memory budget.
    pub budget_bytes: u64,
    /// Host synchronization policy.
    pub host_sync: WorkQueueHostSync,
}

/// Work queue profile where a resident queue should reserve device-side
/// expansion headroom in addition to the initial frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceWorkQueueExpansionProfile {
    /// Initial active work items enqueued before launch.
    pub initial_items: u64,
    /// Additional device-produced work items the queue should absorb when the
    /// explicit queue budget leaves enough room.
    pub expansion_items: u64,
    /// ABI bytes per queue entry.
    pub entry_bytes: u64,
    /// Bytes required for queue head/tail counters and changed flags.
    pub control_bytes: u64,
    /// Caller-approved device-memory budget for the resident queue.
    pub budget_bytes: u64,
    /// Host synchronization policy.
    pub host_sync: WorkQueueHostSync,
}

/// Device-side work queue execution plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceWorkQueuePlan {
    /// Resident queue bytes.
    pub queue_bytes: u64,
    /// Resident control bytes.
    pub control_bytes: u64,
    /// Total resident bytes.
    pub resident_bytes: u64,
    /// Queue occupancy in basis points before device-side expansion.
    pub initial_occupancy_bps: u32,
    /// Whether the plan guarantees final-state-only host synchronization.
    pub final_only_host_sync: bool,
}

/// Device-side work queue drain strategy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceWorkQueueDrainStrategy {
    /// One resident drain window covers the whole queue.
    SingleResidentDrain,
    /// Queue capacity is split into multiple resident drain windows to bound
    /// per-launch queue pressure without host participation.
    ChunkedResidentDrain,
}

/// Device-side work queue plan with bounded resident drain windows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceWorkQueueBackpressurePlan {
    /// Base resident queue byte plan.
    pub queue: DeviceWorkQueuePlan,
    /// Selected resident drain strategy.
    pub strategy: DeviceWorkQueueDrainStrategy,
    /// Maximum queue entries drained by one device-side window.
    pub items_per_chunk: u64,
    /// Number of resident drain windows required to cover queue capacity.
    pub chunks: u64,
    /// Whether the backpressure plan preserves final-state-only host sync.
    pub final_only_host_sync: bool,
}

/// Device work queue planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceWorkQueueError {
    /// Queue capacity must be non-zero.
    ZeroCapacity,
    /// Entry ABI width must be explicit and non-zero.
    ZeroEntryBytes,
    /// Device-side drain chunk size must be non-zero.
    ZeroDrainChunk,
    /// Initial queue contents exceed capacity.
    InitialItemsExceedCapacity {
        /// Initial active items.
        initial_items: u64,
        /// Queue capacity.
        queue_capacity: u64,
    },
    /// Host participation would reintroduce CPU orchestration.
    HostParticipationRejected,
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Queue does not fit the explicit device budget.
    OverBudget {
        /// Required bytes.
        required_bytes: u64,
        /// Budget bytes.
        budget_bytes: u64,
    },
}

impl std::fmt::Display for DeviceWorkQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroCapacity => write!(
                f,
                "device work queue capacity is zero. Fix: size the resident queue before launch."
            ),
            Self::ZeroEntryBytes => write!(
                f,
                "device work queue entry_bytes is zero. Fix: pass the concrete queue-entry ABI width."
            ),
            Self::ZeroDrainChunk => write!(
                f,
                "device work queue drain chunk is zero. Fix: pass a non-zero device-side drain window."
            ),
            Self::InitialItemsExceedCapacity {
                initial_items,
                queue_capacity,
            } => write!(
                f,
                "device work queue initial_items={initial_items} exceeds queue_capacity={queue_capacity}. Fix: shard initial frontier items or increase explicit queue capacity."
            ),
            Self::HostParticipationRejected => write!(
                f,
                "device work queue rejected host participation. Fix: use final-only completion readback so dependent dataflow stays device-side."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "device work queue overflowed while computing {field}. Fix: shard the dependent dataflow workload before queue planning."
            ),
            Self::OverBudget {
                required_bytes,
                budget_bytes,
            } => write!(
                f,
                "device work queue requires {required_bytes} bytes but budget allows {budget_bytes}. Fix: reduce queue capacity, shard the graph, or raise the explicit device budget."
            ),
        }
    }
}

impl std::error::Error for DeviceWorkQueueError {}

fn checked_add(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, DeviceWorkQueueError> {
    lhs.checked_add(rhs)
        .ok_or(DeviceWorkQueueError::ByteCountOverflow { field })
}

fn checked_mul(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, DeviceWorkQueueError> {
    lhs.checked_mul(rhs)
        .ok_or(DeviceWorkQueueError::ByteCountOverflow { field })
}

/// Plan a device-resident work queue for dependent dataflow execution.
pub fn plan_device_work_queue(
    profile: DeviceWorkQueueProfile,
) -> Result<DeviceWorkQueuePlan, DeviceWorkQueueError> {
    if profile.queue_capacity == 0 {
        return Err(DeviceWorkQueueError::ZeroCapacity);
    }
    if profile.entry_bytes == 0 {
        return Err(DeviceWorkQueueError::ZeroEntryBytes);
    }
    if profile.initial_items > profile.queue_capacity {
        return Err(DeviceWorkQueueError::InitialItemsExceedCapacity {
            initial_items: profile.initial_items,
            queue_capacity: profile.queue_capacity,
        });
    }
    if profile.host_sync != WorkQueueHostSync::FinalOnly {
        return Err(DeviceWorkQueueError::HostParticipationRejected);
    }

    let queue_bytes = checked_mul(profile.queue_capacity, profile.entry_bytes, "queue bytes")?;
    let resident_bytes = checked_add(queue_bytes, profile.control_bytes, "resident bytes")?;
    if resident_bytes > profile.budget_bytes {
        return Err(DeviceWorkQueueError::OverBudget {
            required_bytes: resident_bytes,
            budget_bytes: profile.budget_bytes,
        });
    }
    let initial_occupancy_bps = DEVICE_WORK_QUEUE_NUMERIC.ratio_basis_points_u64(
        profile.initial_items,
        profile.queue_capacity,
        0,
        "device work queue initial occupancy",
    );

    Ok(DeviceWorkQueuePlan {
        queue_bytes,
        control_bytes: profile.control_bytes,
        resident_bytes,
        initial_occupancy_bps,
        final_only_host_sync: true,
    })
}

/// Plan a device-resident work queue that preserves initial-frontier capacity
/// and uses remaining queue budget for device-side expansion headroom.
pub fn plan_device_work_queue_with_expansion(
    profile: DeviceWorkQueueExpansionProfile,
) -> Result<DeviceWorkQueuePlan, DeviceWorkQueueError> {
    let desired_capacity = checked_add(
        profile.initial_items,
        profile.expansion_items,
        "queue expansion capacity",
    )?;
    if profile.entry_bytes == 0 {
        return plan_device_work_queue(DeviceWorkQueueProfile {
            initial_items: profile.initial_items,
            queue_capacity: desired_capacity,
            entry_bytes: profile.entry_bytes,
            control_bytes: profile.control_bytes,
            budget_bytes: profile.budget_bytes,
            host_sync: profile.host_sync,
        });
    }
    let budget_capacity =
        profile.budget_bytes.saturating_sub(profile.control_bytes) / profile.entry_bytes;
    let queue_capacity = desired_capacity
        .min(budget_capacity)
        .max(profile.initial_items);
    plan_device_work_queue(DeviceWorkQueueProfile {
        initial_items: profile.initial_items,
        queue_capacity,
        entry_bytes: profile.entry_bytes,
        control_bytes: profile.control_bytes,
        budget_bytes: profile.budget_bytes,
        host_sync: profile.host_sync,
    })
}

/// Plan a device-resident work queue plus bounded device-side drain windows.
pub fn plan_device_work_queue_backpressure(
    profile: DeviceWorkQueueProfile,
    max_items_per_drain_launch: u64,
) -> Result<DeviceWorkQueueBackpressurePlan, DeviceWorkQueueError> {
    if max_items_per_drain_launch == 0 {
        return Err(DeviceWorkQueueError::ZeroDrainChunk);
    }
    let queue = plan_device_work_queue(profile)?;
    let chunks = div_ceil_u64(
        profile.queue_capacity,
        max_items_per_drain_launch,
        "drain chunks",
    )?;
    let strategy = if chunks == 1 {
        DeviceWorkQueueDrainStrategy::SingleResidentDrain
    } else {
        DeviceWorkQueueDrainStrategy::ChunkedResidentDrain
    };
    Ok(DeviceWorkQueueBackpressurePlan {
        queue,
        strategy,
        items_per_chunk: max_items_per_drain_launch.min(profile.queue_capacity),
        chunks,
        final_only_host_sync: true,
    })
}

fn div_ceil_u64(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, DeviceWorkQueueError> {
    DEVICE_WORK_QUEUE_NUMERIC
        .checked_ceil_div_u64(lhs, rhs)
        .ok_or(DeviceWorkQueueError::ByteCountOverflow { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_work_queue_uses_shared_driver_numeric_policy() {
        let source = include_str!("device_work_queue.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: device work-queue source must contain production section");

        assert!(source.contains("BackendNumericPolicy::new"));
        assert!(source.contains("DEVICE_WORK_QUEUE_NUMERIC"));
        assert!(source.contains("checked_ceil_div_u64"));
        assert!(production.contains("fn checked_mul("));
        assert!(production.contains("fn checked_add("));
        assert!(!production.contains("CudaArithmeticOverflow"));
    }

    #[test]
    fn device_work_queue_plans_final_only_resident_execution() {
        let plan = plan_device_work_queue(DeviceWorkQueueProfile {
            initial_items: 256,
            queue_capacity: 1_024,
            entry_bytes: 16,
            control_bytes: 128,
            budget_bytes: 32_768,
            host_sync: WorkQueueHostSync::FinalOnly,
        })
        .expect("Fix: valid device work queue should plan");

        assert_eq!(plan.queue_bytes, 16_384);
        assert_eq!(plan.control_bytes, 128);
        assert_eq!(plan.resident_bytes, 16_512);
        assert_eq!(plan.initial_occupancy_bps, 2_500);
        assert!(plan.final_only_host_sync);
    }

    #[test]
    fn device_work_queue_expansion_uses_budgeted_resident_headroom() {
        let plan = plan_device_work_queue_with_expansion(DeviceWorkQueueExpansionProfile {
            initial_items: 4,
            expansion_items: 12,
            entry_bytes: 8,
            control_bytes: 64,
            budget_bytes: 256,
            host_sync: WorkQueueHostSync::FinalOnly,
        })
        .expect("Fix: expansion headroom should fit inside the explicit queue budget");

        assert_eq!(plan.queue_bytes, 128);
        assert_eq!(plan.control_bytes, 64);
        assert_eq!(plan.resident_bytes, 192);
        assert_eq!(
            plan.initial_occupancy_bps, 2_500,
            "Fix: occupancy must use the expanded resident queue capacity"
        );
        assert!(plan.final_only_host_sync);
    }

    #[test]
    fn device_work_queue_expansion_clamps_to_budget_without_dropping_initial_items() {
        let plan = plan_device_work_queue_with_expansion(DeviceWorkQueueExpansionProfile {
            initial_items: 4,
            expansion_items: 100,
            entry_bytes: 8,
            control_bytes: 16,
            budget_bytes: 96,
            host_sync: WorkQueueHostSync::FinalOnly,
        })
        .expect("Fix: queue expansion should use all affordable headroom");

        assert_eq!(plan.queue_bytes, 80);
        assert_eq!(plan.resident_bytes, 96);
        assert_eq!(
            plan.initial_occupancy_bps, 4_000,
            "Fix: initial occupancy should reflect budget-clamped expansion capacity"
        );
    }

    #[test]
    fn device_work_queue_expansion_fails_when_initial_frontier_cannot_fit() {
        assert_eq!(
            plan_device_work_queue_with_expansion(DeviceWorkQueueExpansionProfile {
                initial_items: 8,
                expansion_items: 100,
                entry_bytes: 16,
                control_bytes: 64,
                budget_bytes: 128,
                host_sync: WorkQueueHostSync::FinalOnly,
            })
            .expect_err("initial frontier must fail when it cannot fit the explicit budget"),
            DeviceWorkQueueError::OverBudget {
                required_bytes: 192,
                budget_bytes: 128,
            }
        );
    }

    #[test]
    fn device_work_queue_expansion_rejects_capacity_overflow() {
        assert_eq!(
            plan_device_work_queue_with_expansion(DeviceWorkQueueExpansionProfile {
                initial_items: u64::MAX,
                expansion_items: 1,
                entry_bytes: 1,
                control_bytes: 0,
                budget_bytes: u64::MAX,
                host_sync: WorkQueueHostSync::FinalOnly,
            })
            .expect_err("overflowed expansion capacity must fail before queue planning"),
            DeviceWorkQueueError::ByteCountOverflow {
                field: "queue expansion capacity",
            }
        );
    }

    #[test]
    fn device_work_queue_rejects_host_participation() {
        assert_eq!(
            plan_device_work_queue(DeviceWorkQueueProfile {
                initial_items: 1,
                queue_capacity: 8,
                entry_bytes: 16,
                control_bytes: 64,
                budget_bytes: 1_024,
                host_sync: WorkQueueHostSync::HostParticipates,
            })
            .expect_err("host participation should fail"),
            DeviceWorkQueueError::HostParticipationRejected
        );
    }

    #[test]
    fn device_work_queue_rejects_invalid_capacity_and_budget() {
        assert_eq!(
            plan_device_work_queue(DeviceWorkQueueProfile {
                initial_items: 9,
                queue_capacity: 8,
                entry_bytes: 16,
                control_bytes: 64,
                budget_bytes: 1_024,
                host_sync: WorkQueueHostSync::FinalOnly,
            })
            .expect_err("initial overflow should fail"),
            DeviceWorkQueueError::InitialItemsExceedCapacity {
                initial_items: 9,
                queue_capacity: 8,
            }
        );
        assert_eq!(
            plan_device_work_queue(DeviceWorkQueueProfile {
                initial_items: 1,
                queue_capacity: 8,
                entry_bytes: 16,
                control_bytes: 64,
                budget_bytes: 128,
                host_sync: WorkQueueHostSync::FinalOnly,
            })
            .expect_err("over-budget queue should fail"),
            DeviceWorkQueueError::OverBudget {
                required_bytes: 192,
                budget_bytes: 128,
            }
        );
    }

    #[test]
    fn device_work_queue_occupancy_uses_widened_arithmetic_for_huge_queues() {
        let plan = plan_device_work_queue(DeviceWorkQueueProfile {
            initial_items: u64::MAX,
            queue_capacity: u64::MAX,
            entry_bytes: 1,
            control_bytes: 0,
            budget_bytes: u64::MAX,
            host_sync: WorkQueueHostSync::FinalOnly,
        })
        .expect("Fix: max-sized byte queue should fit exactly");

        assert_eq!(
            plan.initial_occupancy_bps, 10_000,
            "Fix: device work-queue occupancy must not use saturating u64 multiplication before division; full queues must report 10000 bps even near u64::MAX."
        );
    }

    #[test]
    fn device_work_queue_occupancy_uses_shared_numeric_helper() {
        let source = include_str!("device_work_queue.rs");

        assert!(
            source.contains(concat!("DEVICE_WORK_QUEUE_NUMERIC.", "ratio_basis_points_u64")),
            "Fix: device work-queue occupancy must use the shared driver numeric ratio helper instead of a backend-local basis-point formula."
        );
    }

    #[test]
    fn device_work_queue_backpressure_chunks_large_resident_queues_without_host_participation() {
        let plan = plan_device_work_queue_backpressure(
            DeviceWorkQueueProfile {
                initial_items: 4_096,
                queue_capacity: 65_536,
                entry_bytes: 16,
                control_bytes: 128,
                budget_bytes: 2 << 20,
                host_sync: WorkQueueHostSync::FinalOnly,
            },
            8_192,
        )
        .expect("Fix: large resident work queue should plan bounded device-side drain chunks");

        assert_eq!(
            plan.strategy,
            DeviceWorkQueueDrainStrategy::ChunkedResidentDrain
        );
        assert_eq!(plan.items_per_chunk, 8_192);
        assert_eq!(plan.chunks, 8);
        assert_eq!(plan.queue.resident_bytes, 1_048_704);
        assert!(plan.final_only_host_sync);
        assert!(plan.queue.final_only_host_sync);
    }

    #[test]
    fn device_work_queue_backpressure_ceil_division_handles_max_capacity() {
        let plan = plan_device_work_queue_backpressure(
            DeviceWorkQueueProfile {
                initial_items: u64::MAX,
                queue_capacity: u64::MAX,
                entry_bytes: 1,
                control_bytes: 0,
                budget_bytes: u64::MAX,
                host_sync: WorkQueueHostSync::FinalOnly,
            },
            65_536,
        )
        .expect("Fix: ceil division for max-capacity queues must not overflow");

        assert_eq!(
            plan.strategy,
            DeviceWorkQueueDrainStrategy::ChunkedResidentDrain
        );
        assert_eq!(plan.queue.queue_bytes, u64::MAX);
        assert_eq!(plan.items_per_chunk, 65_536);
        assert_eq!(plan.chunks, 281_474_976_710_656);
        assert!(plan.final_only_host_sync);
    }

    #[test]
    fn device_work_queue_backpressure_rejects_zero_drain_chunk() {
        let err = plan_device_work_queue_backpressure(
            DeviceWorkQueueProfile {
                initial_items: 1,
                queue_capacity: 8,
                entry_bytes: 16,
                control_bytes: 64,
                budget_bytes: 1_024,
                host_sync: WorkQueueHostSync::FinalOnly,
            },
            0,
        )
        .expect_err("zero drain chunk must fail loudly");

        assert_eq!(err, DeviceWorkQueueError::ZeroDrainChunk);
    }

    #[test]
    fn generated_device_work_queue_profiles_preserve_budget_and_sync_contracts() {
        let mut state = 0xa409_3822_299f_31d0_u64;
        for case_index in 0..2048usize {
            let queue_capacity = 1 + next_u64(&mut state) % 262_144;
            let entry_bytes = 1 + next_u64(&mut state) % 256;
            let initial_items = next_u64(&mut state) % (queue_capacity + 1);
            let control_bytes = next_u64(&mut state) % 4096;
            let queue_bytes = queue_capacity
                .checked_mul(entry_bytes)
                .expect("Fix: generated queue byte count should fit");
            let resident_bytes = queue_bytes
                .checked_add(control_bytes)
                .expect("Fix: generated resident byte count should fit");
            let budget_bytes = resident_bytes + (next_u64(&mut state) % 8192);
            let profile = DeviceWorkQueueProfile {
                initial_items,
                queue_capacity,
                entry_bytes,
                control_bytes,
                budget_bytes,
                host_sync: WorkQueueHostSync::FinalOnly,
            };

            let plan = plan_device_work_queue(profile)
                .expect("Fix: generated valid queue profile must plan");
            assert_eq!(plan.queue_bytes, queue_bytes, "case {case_index}");
            assert_eq!(plan.control_bytes, control_bytes, "case {case_index}");
            assert_eq!(plan.resident_bytes, resident_bytes, "case {case_index}");
            assert!(plan.resident_bytes <= budget_bytes, "case {case_index}");
            assert!(plan.initial_occupancy_bps <= 10_000, "case {case_index}");
            assert!(plan.final_only_host_sync, "case {case_index}");

            let drain = 1 + next_u64(&mut state) % queue_capacity;
            let backpressure = plan_device_work_queue_backpressure(profile, drain)
                .expect("Fix: generated valid backpressure profile must plan");
            assert_eq!(backpressure.queue, plan, "case {case_index}");
            assert!(
                backpressure.items_per_chunk <= queue_capacity,
                "case {case_index}"
            );
            assert!(backpressure.chunks >= 1, "case {case_index}");
            assert!(backpressure.final_only_host_sync, "case {case_index}");

            let expansion_items = next_u64(&mut state) % queue_capacity;
            let expansion_budget = resident_bytes + (expansion_items * entry_bytes);
            let expansion =
                plan_device_work_queue_with_expansion(DeviceWorkQueueExpansionProfile {
                    initial_items,
                    expansion_items,
                    entry_bytes,
                    control_bytes,
                    budget_bytes: expansion_budget,
                    host_sync: WorkQueueHostSync::FinalOnly,
                })
                .expect("Fix: generated valid expansion queue profile must plan");
            assert!(
                expansion.resident_bytes <= expansion_budget,
                "case {case_index}"
            );
            assert!(
                expansion.queue_bytes >= initial_items * entry_bytes,
                "case {case_index}"
            );
            assert!(expansion.final_only_host_sync, "case {case_index}");
        }
    }

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }
}
