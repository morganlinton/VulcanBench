//! Device-memory budget enforcement for execution planning.
//!
//! This module owns the budget decision. Buffer enumeration stays in
//! `execution_plan::memory_plan`; backends then receive an already-bounded
//! plan instead of discovering oversize allocations during dispatch.

use super::{MemoryPlan, PlanError};
use crate::optimizer::AdapterCaps;

/// Device-memory limits used by the execution planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceMemoryBudget {
    /// Backend identifier used in diagnostics.
    pub backend: &'static str,
    /// Maximum bytes allowed for one storage/output/uniform buffer.
    pub per_buffer_bytes: u64,
    /// Maximum statically planned bytes across all buffers in one Program.
    pub peak_static_bytes: u64,
}

impl DeviceMemoryBudget {
    /// Build a conservative budget from adapter capabilities.
    #[must_use]
    pub fn from_adapter(caps: &AdapterCaps) -> Self {
        let per_buffer_bytes = caps.max_storage_buffer_binding_size.max(1);
        Self {
            backend: caps.backend,
            per_buffer_bytes,
            peak_static_bytes: per_buffer_bytes.saturating_mul(16),
        }
    }

    /// Validate a memory plan against this budget.
    ///
    /// # Errors
    /// Returns [`PlanError`] with an actionable fix when a single buffer or
    /// the Program's peak static memory exceeds the selected adapter budget.
    pub fn validate(&self, plan: &MemoryPlan) -> Result<MemoryBudgetReport, PlanError> {
        let mut largest_buffer_name = "";
        let mut largest_buffer_bytes = 0u64;
        for buffer in &plan.buffers {
            let Some(size_bytes) = buffer.static_size_bytes else {
                continue;
            };
            if size_bytes > largest_buffer_bytes {
                largest_buffer_name = &buffer.name;
                largest_buffer_bytes = size_bytes;
            }
            if size_bytes > self.per_buffer_bytes {
                return Err(PlanError::BufferBudgetExceeded {
                    backend: self.backend,
                    name: buffer.name.clone(),
                    size_bytes,
                    budget_bytes: self.per_buffer_bytes,
                });
            }
        }
        if plan.static_bytes > self.peak_static_bytes {
            return Err(PlanError::PeakBudgetExceeded {
                backend: self.backend,
                planned_bytes: plan.static_bytes,
                budget_bytes: self.peak_static_bytes,
            });
        }
        Ok(MemoryBudgetReport {
            backend: self.backend,
            static_bytes: plan.static_bytes,
            peak_budget_bytes: self.peak_static_bytes,
            largest_buffer_name: largest_buffer_name.to_owned(),
            largest_buffer_bytes,
            per_buffer_budget_bytes: self.per_buffer_bytes,
            dynamic_buffers: plan.dynamic_buffers,
        })
    }
}

/// Successful memory-budget validation report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryBudgetReport {
    /// Backend identifier used for the budget.
    pub backend: &'static str,
    /// Program static bytes.
    pub static_bytes: u64,
    /// Peak budget used for validation.
    pub peak_budget_bytes: u64,
    /// Name of the largest statically sized buffer, or empty for none.
    pub largest_buffer_name: String,
    /// Byte size of the largest statically sized buffer.
    pub largest_buffer_bytes: u64,
    /// Per-buffer byte budget.
    pub per_buffer_budget_bytes: u64,
    /// Count of runtime-sized buffers in the plan.
    pub dynamic_buffers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_plan::{BufferPlan, MemoryPlan};
    use crate::ir::{BufferAccess, DataType, MemoryKind};

    fn plan(static_sizes: &[u64]) -> MemoryPlan {
        MemoryPlan {
            buffers: static_sizes
                .iter()
                .enumerate()
                .map(|(index, &size)| BufferPlan {
                    name: format!("b{index}"),
                    binding: index as u32,
                    access: BufferAccess::ReadWrite,
                    kind: MemoryKind::Global,
                    element: DataType::U32,
                    count: (size / 4) as u32,
                    static_size_bytes: Some(size),
                    output_range: None,
                })
                .collect(),
            static_bytes: static_sizes.iter().copied().sum(),
            dynamic_buffers: 0,
            visible_readback_bytes: 0,
            avoided_readback_bytes: 0,
        }
    }

    #[test]
    fn validates_under_budget_and_reports_largest_buffer() {
        let budget = DeviceMemoryBudget {
            backend: "test-gpu",
            per_buffer_bytes: 1024,
            peak_static_bytes: 4096,
        };
        let report = budget
            .validate(&plan(&[128, 512, 256]))
            .expect("Fix: plan is below both per-buffer and peak budgets");
        assert_eq!(report.backend, "test-gpu");
        assert_eq!(report.static_bytes, 896);
        assert_eq!(report.largest_buffer_name, "b1");
        assert_eq!(report.largest_buffer_bytes, 512);
    }

    #[test]
    fn rejects_single_buffer_over_budget_before_peak_check() {
        let budget = DeviceMemoryBudget {
            backend: "test-gpu",
            per_buffer_bytes: 1024,
            peak_static_bytes: 4096,
        };
        let error = budget
            .validate(&plan(&[128, 2048]))
            .expect_err("single oversize buffer must fail before dispatch");
        assert!(matches!(
            error,
            PlanError::BufferBudgetExceeded {
                backend: "test-gpu",
                name,
                size_bytes: 2048,
                budget_bytes: 1024,
            } if name == "b1"
        ));
    }

    #[test]
    fn rejects_peak_static_bytes_over_budget() {
        let budget = DeviceMemoryBudget {
            backend: "test-gpu",
            per_buffer_bytes: 1024,
            peak_static_bytes: 1500,
        };
        let error = budget
            .validate(&plan(&[800, 800]))
            .expect_err("aggregate peak memory must be bounded");
        assert!(matches!(
            error,
            PlanError::PeakBudgetExceeded {
                backend: "test-gpu",
                planned_bytes: 1600,
                budget_bytes: 1500,
            }
        ));
    }
}
