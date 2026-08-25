//! Adapter from substrate frontier-typed IR plans to CUDA frontier waves.

use crate::backend::accounting::{checked_mul_u64_count as checked_mul, CudaArithmeticOverflow};
use crate::backend::staging_reserve::{
    reserve_typed_vec as reserve_vec, CudaStorageReserveFailure,
};
use crate::megakernel_barrier_planner::{CudaMegakernelFrontierWave, CudaMegakernelWaveDependency};
use vyre_self_substrate::frontier_typed_ir::FrontierTypedPlan;

/// CUDA-ready frontier wave input derived from a frontier-typed IR plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaFrontierTypedIrInput {
    /// CUDA wave byte envelopes.
    pub waves: Vec<CudaMegakernelFrontierWave>,
    /// Active work items per CUDA frontier wave.
    pub active_items: Vec<u64>,
    /// Dependencies preserving frontier-typed wave order.
    pub dependencies: Vec<CudaMegakernelWaveDependency>,
}

impl CudaFrontierTypedIrInput {
    /// Create an empty CUDA frontier input.
    #[must_use]
    pub fn new() -> Self {
        Self {
            waves: Vec::new(),
            active_items: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Create an empty CUDA frontier input with fallibly reserved storage for `wave_count`.
    pub fn try_with_capacity(wave_count: usize) -> Result<Self, CudaFrontierTypedIrAdapterError> {
        let mut input = Self::new();
        input.try_reserve_for_waves(wave_count)?;
        Ok(input)
    }

    fn clear_preserving_capacity(&mut self) {
        self.waves.clear();
        self.active_items.clear();
        self.dependencies.clear();
    }

    fn try_reserve_for_waves(
        &mut self,
        wave_count: usize,
    ) -> Result<(), CudaFrontierTypedIrAdapterError> {
        let dependency_count = dependency_capacity(wave_count);
        reserve_vec(&mut self.waves, wave_count, "waves")?;
        reserve_vec(&mut self.active_items, wave_count, "active items")?;
        reserve_vec(&mut self.dependencies, dependency_count, "dependencies")?;
        Ok(())
    }
}

impl Default for CudaFrontierTypedIrInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors produced while adapting frontier-typed IR to CUDA frontier waves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaFrontierTypedIrAdapterError {
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Host-side staging storage could not be reserved.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Number of elements requested.
        requested: usize,
        /// Allocator error text.
        message: String,
    },
}

impl CudaArithmeticOverflow for CudaFrontierTypedIrAdapterError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl CudaStorageReserveFailure for CudaFrontierTypedIrAdapterError {
    fn storage_reserve_failed(field: &'static str, requested: usize, message: String) -> Self {
        Self::StorageReserveFailed {
            field,
            requested,
            message,
        }
    }
}

impl std::fmt::Display for CudaFrontierTypedIrAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ByteCountOverflow { field } => write!(
                f,
                "frontier-typed IR CUDA adapter overflowed while computing {field}. Fix: shard the frontier plan before CUDA megakernel planning."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "frontier-typed IR CUDA adapter could not reserve {requested} {field} entries: {message}. Fix: shard the frontier plan before CUDA megakernel planning."
            ),
        }
    }
}

impl std::error::Error for CudaFrontierTypedIrAdapterError {}

/// Convert a frontier-typed IR plan into CUDA frontier execution input.
pub fn adapt_frontier_typed_ir_to_cuda(
    plan: &FrontierTypedPlan,
    frontier_bytes_per_active_item: u64,
    scratch_bytes_per_active_item: u64,
    output_bytes_per_wave: u64,
) -> Result<CudaFrontierTypedIrInput, CudaFrontierTypedIrAdapterError> {
    let mut out = CudaFrontierTypedIrInput::try_with_capacity(plan.waves.len())?;
    adapt_frontier_typed_ir_to_cuda_into(
        plan,
        frontier_bytes_per_active_item,
        scratch_bytes_per_active_item,
        output_bytes_per_wave,
        &mut out,
    )?;
    Ok(out)
}

/// Convert a frontier-typed IR plan into caller-owned CUDA frontier input storage.
pub fn adapt_frontier_typed_ir_to_cuda_into(
    plan: &FrontierTypedPlan,
    frontier_bytes_per_active_item: u64,
    scratch_bytes_per_active_item: u64,
    output_bytes_per_wave: u64,
    out: &mut CudaFrontierTypedIrInput,
) -> Result<(), CudaFrontierTypedIrAdapterError> {
    for wave in &plan.waves {
        checked_mul(
            wave.active_items,
            frontier_bytes_per_active_item,
            "frontier bytes",
        )?;
        checked_mul(
            wave.active_items,
            scratch_bytes_per_active_item,
            "scratch bytes",
        )?;
    }

    out.clear_preserving_capacity();
    out.try_reserve_for_waves(plan.waves.len())?;
    for wave in &plan.waves {
        out.active_items.push(wave.active_items);
        out.waves.push(CudaMegakernelFrontierWave {
            frontier_bytes: wave.active_items * frontier_bytes_per_active_item,
            scratch_bytes: wave.active_items * scratch_bytes_per_active_item,
            output_bytes: output_bytes_per_wave,
        });
    }
    for index in 1..plan.waves.len() {
        out.dependencies.push(CudaMegakernelWaveDependency {
            before: index - 1,
            after: index,
        });
    }
    Ok(())
}

const fn dependency_capacity(wave_count: usize) -> usize {
    match wave_count {
        0 => 0,
        count => count - 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::megakernel_barrier_planner::plan_cuda_megakernel_barriers;
    use vyre_self_substrate::frontier_typed_ir::{
        plan_frontier_typed_ir, FrontierDependency, FrontierDomain, FrontierNode,
    };

    #[test]
    fn adapter_converts_frontier_waves_to_cuda_byte_envelopes() {
        let plan = plan_frontier_typed_ir(
            &[
                FrontierNode {
                    id: 0,
                    domain: FrontierDomain::Parser,
                    active_items: 4,
                },
                FrontierNode {
                    id: 1,
                    domain: FrontierDomain::Semantic,
                    active_items: 6,
                },
            ],
            &[FrontierDependency {
                before: 0,
                after: 1,
            }],
        )
        .expect("Fix: frontier plan should build");

        let cuda = adapt_frontier_typed_ir_to_cuda(&plan, 8, 16, 32)
            .expect("Fix: frontier plan should adapt to CUDA");

        assert_eq!(
            cuda.active_items,
            vec![4, 6],
            "Fix: CUDA frontier adapter must preserve active item counts for device work-queue planning."
        );
        assert_eq!(
            cuda.waves,
            vec![
                CudaMegakernelFrontierWave {
                    frontier_bytes: 32,
                    scratch_bytes: 64,
                    output_bytes: 32,
                },
                CudaMegakernelFrontierWave {
                    frontier_bytes: 48,
                    scratch_bytes: 96,
                    output_bytes: 32,
                },
            ]
        );
        assert_eq!(
            cuda.dependencies,
            vec![CudaMegakernelWaveDependency {
                before: 0,
                after: 1,
            }]
        );
        let barriers = plan_cuda_megakernel_barriers(cuda.waves.len(), &cuda.dependencies)
            .expect("Fix: adapted frontier dependencies should barrier-plan");
        assert_eq!(barriers.global_barriers, 1);
        assert_eq!(barriers.groups[0].waves, vec![0]);
        assert_eq!(barriers.groups[1].waves, vec![1]);
    }

    #[test]
    fn adapter_rejects_overflowing_wave_bytes() {
        let plan = FrontierTypedPlan {
            waves: vec![vyre_self_substrate::frontier_typed_ir::FrontierWave {
                index: 0,
                domains: vec![FrontierDomain::Dataflow],
                node_ids: vec![7],
                active_items: u64::MAX,
            }],
        };

        let err = adapt_frontier_typed_ir_to_cuda(&plan, 2, 1, 0)
            .expect_err("overflowing frontier bytes should fail");
        assert_eq!(
            err,
            CudaFrontierTypedIrAdapterError::ByteCountOverflow {
                field: "frontier bytes",
            }
        );
    }

    #[test]
    fn adapter_into_reuses_caller_owned_frontier_storage() {
        let mut out = CudaFrontierTypedIrInput::try_with_capacity(8)
            .expect("Fix: frontier storage should reserve");
        let initial_wave_capacity = out.waves.capacity();
        let initial_active_capacity = out.active_items.capacity();
        let initial_dependency_capacity = out.dependencies.capacity();
        let plan = plan_frontier_typed_ir(
            &[
                FrontierNode {
                    id: 0,
                    domain: FrontierDomain::Parser,
                    active_items: 4,
                },
                FrontierNode {
                    id: 1,
                    domain: FrontierDomain::Semantic,
                    active_items: 6,
                },
            ],
            &[FrontierDependency {
                before: 0,
                after: 1,
            }],
        )
        .expect("Fix: frontier plan should build");

        adapt_frontier_typed_ir_to_cuda_into(&plan, 8, 16, 32, &mut out)
            .expect("Fix: frontier plan should adapt into reused CUDA storage");

        assert_eq!(out.waves.capacity(), initial_wave_capacity);
        assert_eq!(out.active_items.capacity(), initial_active_capacity);
        assert_eq!(out.dependencies.capacity(), initial_dependency_capacity);
        assert_eq!(out.active_items, vec![4, 6]);
        assert_eq!(out.dependencies.len(), 1);
    }

    #[test]
    fn adapter_builds_cuda_dependencies_without_iterator_collect_staging() {
        let source = include_str!("frontier_typed_ir_adapter.rs");

        assert!(
            source.contains("fn try_reserve_for_waves(")
                && source.contains("reserve_typed_vec as reserve_vec")
                && source.contains("dependencies.push(CudaMegakernelWaveDependency"),
            "Fix: frontier-typed CUDA adapter must build dependency edges with explicit fallible preallocated storage."
        );
        assert!(
            !source.contains(concat!(".map(|index| ", "CudaMegakernelWaveDependency"))
                && !source.contains(concat!(".collect", "();")),
            "Fix: frontier-typed CUDA adapter must not use iterator collect staging on dependency-wave conversion."
        );
    }

    #[test]
    fn adapter_capacity_planning_is_checked_and_fallible() {
        let source = include_str!("frontier_typed_ir_adapter.rs");

        assert!(
            source.contains("dependency_capacity(wave_count)")
                && source.contains("reserve_vec(&mut self.waves, wave_count")
                && source.contains("StorageReserveFailed"),
            "Fix: frontier-typed CUDA adapter must not hide release-path capacity failures behind infallible Vec reservations."
        );
        assert!(
            !source.contains(concat!("wave_count", ".saturating_sub"))
                && !source.contains(concat!("Vec::with_capacity", "(wave_count)"))
                && !source.contains(concat!("reserve", "(wave_count)")),
            "Fix: frontier-typed CUDA adapter must not use saturating or infallible capacity staging in production."
        );
    }
}
