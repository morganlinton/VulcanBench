//! Backend-neutral frontier memory planning for dependency-aware megakernels.
//!
//! Backends can choose different execution topologies, but the memory envelope
//! of dependency-layered frontier waves is a backend-neutral contract. This
//! module plans that envelope once, including dependency barriers, fused-group
//! splitting under an explicit byte budget, peak byte accounting, and readback
//! pressure amortization.

use crate::accounting::{
    checked_add_u64_count as checked_add, checked_mul_u64_count as checked_mul,
};
use crate::megakernel_barrier::{
    plan_megakernel_barriers_with_scratch, MegakernelBarrierGroup, MegakernelBarrierPlan,
    MegakernelBarrierPlanError, MegakernelBarrierScratch, MegakernelWaveDependency,
};
use crate::reservation_policy::{
    reserve_typed_vec_to_capacity as reserve_vec_to_capacity, ReservationPolicy,
};

const MEGAKERNEL_FRONTIER_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "megakernel frontier memory planner",
    "shard the frontier wave group or split the fused phase",
);

/// Frontier-typed megakernel wave memory envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MegakernelFrontierWave {
    /// Resident frontier bytes touched by this wave.
    pub frontier_bytes: u64,
    /// Temporary scratch bytes required by this wave before topology scaling.
    pub scratch_bytes: u64,
    /// Output bytes produced by this wave.
    pub output_bytes: u64,
}

/// Dependency-aware megakernel frontier memory plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MegakernelFrontierMemoryPlan {
    /// Minimum global-barrier grouping after memory-budget splitting.
    pub barriers: MegakernelBarrierPlan,
    /// Peak frontier bytes across any fused barrier-free group.
    pub peak_frontier_bytes: u64,
    /// Peak scratch bytes across any fused barrier-free group.
    pub peak_scratch_bytes: u64,
    /// Peak output bytes across any fused barrier-free group.
    pub peak_output_bytes: u64,
    /// Readback pressure after combining runtime telemetry with static
    /// fused-wave output volume.
    pub amortized_readback_bytes: u64,
    /// Widest barrier-free group in wave count.
    pub max_group_width: usize,
}

/// Frontier memory planning failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MegakernelFrontierMemoryPlanError {
    /// Dependency graph cannot be barrier-planned.
    Barrier(MegakernelBarrierPlanError),
    /// Peak wave bytes overflowed while grouping a barrier-free phase.
    ByteCountOverflow {
        /// Field being accumulated.
        field: &'static str,
    },
    /// Static graph or fused frontier bytes exceed the caller-approved budget.
    GroupOverBudget {
        /// Required bytes before topology selection.
        required_bytes: u64,
        /// Caller-provided budget.
        budget_bytes: u64,
        /// Budget region being checked.
        field: &'static str,
    },
    /// Frontier planning result storage could not be reserved.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Number of elements requested.
        requested: usize,
        /// Allocator error text.
        message: String,
    },
}

impl crate::accounting::ArithmeticOverflow for MegakernelFrontierMemoryPlanError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl std::fmt::Display for MegakernelFrontierMemoryPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Barrier(error) => error.fmt(f),
            Self::ByteCountOverflow { field } => write!(
                f,
                "megakernel frontier memory planner overflowed while accumulating {field}. Fix: shard the frontier wave group or split the fused phase."
            ),
            Self::GroupOverBudget {
                required_bytes,
                budget_bytes,
                field,
            } => write!(
                f,
                "megakernel frontier memory planner requires {required_bytes} bytes for {field} but budget allows {budget_bytes}. Fix: shard the graph/frontier waves or raise the explicit megakernel budget."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "megakernel frontier memory planner could not reserve {requested} {field} entries: {message}. Fix: shard the frontier waves before planning."
            ),
        }
    }
}

impl std::error::Error for MegakernelFrontierMemoryPlanError {}

impl From<MegakernelBarrierPlanError> for MegakernelFrontierMemoryPlanError {
    fn from(error: MegakernelBarrierPlanError) -> Self {
        Self::Barrier(error)
    }
}

/// Plan dependency-aware frontier memory using caller-owned barrier scratch.
///
/// # Errors
///
/// Returns [`MegakernelFrontierMemoryPlanError`] when dependencies are invalid,
/// counters overflow, or the requested graph/frontier envelope cannot fit the
/// explicit budget.
pub fn plan_megakernel_frontier_memory_with_scratch(
    waves: &[MegakernelFrontierWave],
    dependencies: &[MegakernelWaveDependency],
    resident_graph_bytes: u64,
    budget_bytes: u64,
    readback_bytes: u64,
    scratch: &mut MegakernelBarrierScratch,
) -> Result<MegakernelFrontierMemoryPlan, MegakernelFrontierMemoryPlanError> {
    let barriers = plan_megakernel_barriers_with_scratch(waves.len(), dependencies, scratch)?;
    let group_budget_bytes = budget_bytes.checked_sub(resident_graph_bytes).ok_or(
        MegakernelFrontierMemoryPlanError::GroupOverBudget {
            required_bytes: resident_graph_bytes,
            budget_bytes,
            field: "resident graph bytes",
        },
    )?;
    let barriers = split_barrier_groups_to_memory_budget(barriers, waves, group_budget_bytes)?;
    let mut peak_frontier_bytes = 0u64;
    let mut peak_scratch_bytes = 0u64;
    let mut peak_output_bytes = 0u64;
    let mut max_group_width = 0usize;
    for group in &barriers.groups {
        let mut group_frontier_bytes = 0u64;
        let mut group_scratch_bytes = 0u64;
        let mut group_output_bytes = 0u64;
        max_group_width = max_group_width.max(group.waves.len());
        for &wave_index in &group.waves {
            let wave = waves[wave_index];
            group_frontier_bytes = checked_add::<MegakernelFrontierMemoryPlanError>(
                group_frontier_bytes,
                wave.frontier_bytes,
                "frontier wave bytes",
            )?;
            group_scratch_bytes = checked_add::<MegakernelFrontierMemoryPlanError>(
                group_scratch_bytes,
                wave.scratch_bytes,
                "scratch wave bytes",
            )?;
            group_output_bytes = checked_add::<MegakernelFrontierMemoryPlanError>(
                group_output_bytes,
                wave.output_bytes,
                "output wave bytes",
            )?;
        }
        peak_frontier_bytes = peak_frontier_bytes.max(group_frontier_bytes);
        peak_scratch_bytes = peak_scratch_bytes.max(group_scratch_bytes);
        peak_output_bytes = peak_output_bytes.max(group_output_bytes);
    }

    Ok(MegakernelFrontierMemoryPlan {
        barriers,
        peak_frontier_bytes,
        peak_scratch_bytes,
        peak_output_bytes,
        amortized_readback_bytes: readback_bytes.max(peak_output_bytes),
        max_group_width,
    })
}

fn split_barrier_groups_to_memory_budget(
    barriers: MegakernelBarrierPlan,
    waves: &[MegakernelFrontierWave],
    group_budget_bytes: u64,
) -> Result<MegakernelBarrierPlan, MegakernelFrontierMemoryPlanError> {
    let mut groups = Vec::new();
    reserve_vec::<MegakernelBarrierGroup>(
        &mut groups,
        barriers.groups.len(),
        "split barrier groups",
    )?;
    for group in barriers.groups {
        split_one_barrier_group_to_memory_budget(group, waves, group_budget_bytes, &mut groups)?;
    }
    Ok(MegakernelBarrierPlan {
        global_barriers: if groups.is_empty() {
            0
        } else {
            groups.len() - 1
        },
        groups,
    })
}

fn split_one_barrier_group_to_memory_budget(
    group: MegakernelBarrierGroup,
    waves: &[MegakernelFrontierWave],
    group_budget_bytes: u64,
    groups: &mut Vec<MegakernelBarrierGroup>,
) -> Result<(), MegakernelFrontierMemoryPlanError> {
    let mut current = Vec::new();
    reserve_vec::<usize>(
        &mut current,
        group.waves.len().min(8),
        "current split barrier group",
    )?;
    let mut current_bytes = 0u64;
    for wave_index in group.waves {
        let wave_bytes = megakernel_frontier_fused_wave_budget_bytes(waves[wave_index])?;
        let combined = checked_add::<MegakernelFrontierMemoryPlanError>(
            current_bytes,
            wave_bytes,
            "barrier group fused wave budget bytes",
        )?;
        if current.is_empty() && wave_bytes > group_budget_bytes {
            return Err(MegakernelFrontierMemoryPlanError::GroupOverBudget {
                required_bytes: wave_bytes,
                budget_bytes: group_budget_bytes,
                field: "single fused frontier wave bytes",
            });
        }
        if !current.is_empty() && combined > group_budget_bytes {
            groups.push(MegakernelBarrierGroup {
                waves: std::mem::take(&mut current),
            });
            current_bytes = 0;
        }
        current.push(wave_index);
        current_bytes = checked_add::<MegakernelFrontierMemoryPlanError>(
            current_bytes,
            wave_bytes,
            "barrier group fused wave budget bytes",
        )?;
    }
    if !current.is_empty() {
        groups.push(MegakernelBarrierGroup { waves: current });
    }
    Ok(())
}

/// Compute the byte budget used to decide whether one frontier wave can fit in
/// a fused barrier-free resident group.
pub fn megakernel_frontier_fused_wave_budget_bytes(
    wave: MegakernelFrontierWave,
) -> Result<u64, MegakernelFrontierMemoryPlanError> {
    let fused_scratch_bytes = checked_mul::<MegakernelFrontierMemoryPlanError>(
        wave.scratch_bytes,
        4,
        "fused wave scratch bytes",
    )?;
    let bytes = checked_add::<MegakernelFrontierMemoryPlanError>(
        wave.frontier_bytes,
        fused_scratch_bytes,
        "fused wave bytes",
    )?;
    checked_add::<MegakernelFrontierMemoryPlanError>(bytes, wave.output_bytes, "fused wave bytes")
}

fn reserve_vec<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
    item: &'static str,
) -> Result<(), MegakernelFrontierMemoryPlanError> {
    reserve_vec_to_capacity(
        MEGAKERNEL_FRONTIER_RESERVATION,
        vec,
        target_capacity,
        item,
        storage_reserve_failed,
    )
}

fn storage_reserve_failed(
    field: &'static str,
    requested: usize,
    message: String,
) -> MegakernelFrontierMemoryPlanError {
    MegakernelFrontierMemoryPlanError::StorageReserveFailed {
        field,
        requested,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        megakernel_frontier_fused_wave_budget_bytes, plan_megakernel_frontier_memory_with_scratch,
        MegakernelFrontierMemoryPlanError, MegakernelFrontierWave,
    };
    use crate::megakernel_barrier::{MegakernelBarrierScratch, MegakernelWaveDependency};

    #[test]
    fn frontier_memory_plan_uses_peak_barrier_group_memory() {
        let mut scratch = MegakernelBarrierScratch::default();
        let plan = plan_megakernel_frontier_memory_with_scratch(
            &[
                MegakernelFrontierWave {
                    frontier_bytes: 1_024,
                    scratch_bytes: 512,
                    output_bytes: 256,
                },
                MegakernelFrontierWave {
                    frontier_bytes: 2_048,
                    scratch_bytes: 1_024,
                    output_bytes: 512,
                },
                MegakernelFrontierWave {
                    frontier_bytes: 4_096,
                    scratch_bytes: 2_048,
                    output_bytes: 1_024,
                },
                MegakernelFrontierWave {
                    frontier_bytes: 8_192,
                    scratch_bytes: 4_096,
                    output_bytes: 2_048,
                },
            ],
            &[
                MegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                MegakernelWaveDependency {
                    before: 0,
                    after: 2,
                },
                MegakernelWaveDependency {
                    before: 1,
                    after: 3,
                },
                MegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
            16_000,
            128 * 1024,
            1 << 20,
            &mut scratch,
        )
        .expect("Fix: frontier-typed megakernel memory plan should fit the budget.");

        assert_eq!(plan.barriers.global_barriers, 2);
        assert_eq!(plan.barriers.groups[1].waves, vec![1, 2]);
        assert_eq!(plan.peak_frontier_bytes, 8_192);
        assert_eq!(plan.peak_scratch_bytes, 4_096);
        assert_eq!(plan.peak_output_bytes, 2_048);
        assert_eq!(plan.amortized_readback_bytes, 1 << 20);
        assert_eq!(plan.max_group_width, 2);
    }

    #[test]
    fn frontier_memory_uses_static_group_output_to_amortize_readback() {
        let mut scratch = MegakernelBarrierScratch::default();
        let plan = plan_megakernel_frontier_memory_with_scratch(
            &[
                MegakernelFrontierWave {
                    frontier_bytes: 1_024,
                    scratch_bytes: 512,
                    output_bytes: 3_072,
                },
                MegakernelFrontierWave {
                    frontier_bytes: 1_024,
                    scratch_bytes: 512,
                    output_bytes: 3_072,
                },
            ],
            &[],
            16_000,
            128 * 1024,
            0,
            &mut scratch,
        )
        .expect("Fix: static output-amortized frontier memory plan should fit the budget.");

        assert_eq!(plan.peak_output_bytes, 6_144);
        assert_eq!(plan.amortized_readback_bytes, 6_144);
    }

    #[test]
    fn frontier_memory_splits_independent_layers_to_fit_fused_budget() {
        let mut scratch = MegakernelBarrierScratch::default();
        let waves = [
            MegakernelFrontierWave {
                frontier_bytes: 10,
                scratch_bytes: 10,
                output_bytes: 10,
            },
            MegakernelFrontierWave {
                frontier_bytes: 10,
                scratch_bytes: 10,
                output_bytes: 10,
            },
            MegakernelFrontierWave {
                frontier_bytes: 10,
                scratch_bytes: 10,
                output_bytes: 10,
            },
        ];
        let plan =
            plan_megakernel_frontier_memory_with_scratch(&waves, &[], 0, 100, 4_096, &mut scratch)
                .expect("Fix: independent frontier waves should split into budget-fit chunks.");

        assert_eq!(plan.barriers.groups.len(), 3);
        assert_eq!(plan.barriers.global_barriers, 2);
        assert_eq!(plan.max_group_width, 1);
        assert_eq!(plan.peak_frontier_bytes, 10);
        assert_eq!(plan.peak_scratch_bytes, 10);
        assert_eq!(plan.peak_output_bytes, 10);
    }

    #[test]
    fn frontier_memory_rejects_graph_and_single_wave_over_budget() {
        let mut scratch = MegakernelBarrierScratch::default();
        let graph_error = plan_megakernel_frontier_memory_with_scratch(
            &[MegakernelFrontierWave {
                frontier_bytes: 1,
                scratch_bytes: 1,
                output_bytes: 1,
            }],
            &[],
            1_600,
            1_000,
            0,
            &mut scratch,
        )
        .expect_err("resident graph bytes above budget must fail before split planning");
        assert_eq!(
            graph_error,
            MegakernelFrontierMemoryPlanError::GroupOverBudget {
                required_bytes: 1_600,
                budget_bytes: 1_000,
                field: "resident graph bytes",
            }
        );

        let wave_error = plan_megakernel_frontier_memory_with_scratch(
            &[MegakernelFrontierWave {
                frontier_bytes: 100,
                scratch_bytes: 100,
                output_bytes: 100,
            }],
            &[],
            0,
            500,
            0,
            &mut scratch,
        )
        .expect_err("single fused wave above group budget must fail before topology planning");
        assert_eq!(
            wave_error,
            MegakernelFrontierMemoryPlanError::GroupOverBudget {
                required_bytes: 600,
                budget_bytes: 500,
                field: "single fused frontier wave bytes",
            }
        );
    }

    #[test]
    fn frontier_fused_wave_budget_uses_topology_scratch_multiplier() {
        assert_eq!(
            megakernel_frontier_fused_wave_budget_bytes(MegakernelFrontierWave {
                frontier_bytes: 16,
                scratch_bytes: 16,
                output_bytes: 16,
            })
            .expect("Fix: fused frontier wave budget should fit"),
            96
        );
    }

    #[test]
    fn frontier_memory_fails_loudly_on_wave_byte_overflow() {
        let mut scratch = MegakernelBarrierScratch::default();
        let error = plan_megakernel_frontier_memory_with_scratch(
            &[
                MegakernelFrontierWave {
                    frontier_bytes: u64::MAX,
                    scratch_bytes: 1,
                    output_bytes: 1,
                },
                MegakernelFrontierWave {
                    frontier_bytes: 1,
                    scratch_bytes: 1,
                    output_bytes: 1,
                },
            ],
            &[],
            2,
            u64::MAX,
            0,
            &mut scratch,
        )
        .expect_err("Fix: overflowed frontier wave bytes must fail before launch planning.");

        assert_eq!(
            error,
            MegakernelFrontierMemoryPlanError::ByteCountOverflow {
                field: "fused wave bytes"
            }
        );
    }

    #[test]
    fn generated_frontier_memory_profiles_preserve_peak_and_budget_for_1024_shapes() {
        let mut scratch = MegakernelBarrierScratch::default();
        for width in 1u64..=32 {
            for depth in 1u64..=32 {
                let mut waves = Vec::new();
                let mut dependencies = Vec::new();
                for layer in 0..depth {
                    for slot in 0..width {
                        waves.push(MegakernelFrontierWave {
                            frontier_bytes: width,
                            scratch_bytes: slot + 1,
                            output_bytes: layer + 1,
                        });
                        if layer + 1 < depth {
                            dependencies.push(MegakernelWaveDependency {
                                before: (layer * width + slot) as usize,
                                after: ((layer + 1) * width + slot) as usize,
                            });
                        }
                    }
                }

                let plan = plan_megakernel_frontier_memory_with_scratch(
                    &waves,
                    &dependencies,
                    256,
                    u64::MAX / 2,
                    7,
                    &mut scratch,
                )
                .expect("Fix: generated frontier memory DAG should plan under large budget.");

                assert_eq!(plan.barriers.groups.len(), depth as usize);
                assert_eq!(plan.max_group_width, width as usize);
                assert_eq!(plan.peak_frontier_bytes, width * width);
                assert_eq!(plan.peak_scratch_bytes, width * (width + 1) / 2);
                assert_eq!(plan.peak_output_bytes, width * depth);
                assert_eq!(plan.amortized_readback_bytes, 7.max(width * depth));
            }
        }
    }
}
