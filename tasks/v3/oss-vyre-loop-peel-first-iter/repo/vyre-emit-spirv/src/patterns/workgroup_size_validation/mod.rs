//! Vulkan workgroup-size limit validation.
//!
//! SPIR-V compute shaders for Vulkan are subject to per-device
//! `VkPhysicalDeviceLimits`:
//!
//! - `maxComputeWorkGroupSize[3]`  -  per-dimension limit. Standard
//!   minimum is `[1024, 1024, 64]`; many drivers go higher.
//! - `maxComputeWorkGroupInvocations`  -  total threads per workgroup
//!   (the product of the three dims). Standard minimum is `1024`.
//!
//! This pattern checks `desc.dispatch.workgroup_size` against the
//! Vulkan-baseline limits AND a configurable per-device profile.
//! Returns a `ValidationReport` with each violation as a separate
//! entry so callers can route them individually.
//!
//! Detection-only: emit happens regardless. The host pipeline
//! builder consults this report to decide whether to refuse the
//! dispatch, fall back to a smaller workgroup_size override, or
//! raise the device requirement bar.

use serde::{Deserialize, Serialize};
use vyre_lower::KernelDescriptor;

/// Vulkan-baseline limits  -  every conformant Vulkan implementation
/// must support at least these. Most desktop GPUs support
/// considerably higher.
pub const VULKAN_BASELINE: DeviceLimits = DeviceLimits {
    max_workgroup_size_per_dim: [1024, 1024, 64],
    max_workgroup_invocations: 1024,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceLimits {
    /// Per-dimension limit (X, Y, Z).
    pub max_workgroup_size_per_dim: [u32; 3],
    /// Product limit  -  total threads per workgroup.
    pub max_workgroup_invocations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Violation {
    /// `workgroup_size[axis]` exceeds `limits.max_workgroup_size_per_dim[axis]`.
    DimExceeded { axis: u8, actual: u32, limit: u32 },
    /// Product `workgroup_size[0] * [1] * [2]` exceeds
    /// `limits.max_workgroup_invocations`.
    InvocationsExceeded { actual: u32, limit: u32 },
    /// One of the dims is zero  -  kernel would never run.
    ZeroDim { axis: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidationReport {
    pub kernel_id: String,
    pub workgroup_size: [u32; 3],
    pub limits: DeviceLimits,
    pub violations: Vec<Violation>,
}

impl ValidationReport {
    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }
    pub fn invocations(&self) -> u32 {
        self.workgroup_size[0]
            .saturating_mul(self.workgroup_size[1])
            .saturating_mul(self.workgroup_size[2])
    }
}

/// Validate against the Vulkan baseline (`VULKAN_BASELINE`).
#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> ValidationReport {
    analyze_against(desc, VULKAN_BASELINE)
}

/// Validate against a custom device profile (use when targeting a
/// specific GPU's known limits, e.g. NVIDIA RTX 4090's
/// `[1024, 1024, 64]` and 1024 invocations  -  same as baseline).
#[must_use]
pub fn analyze_against(desc: &KernelDescriptor, limits: DeviceLimits) -> ValidationReport {
    let wg = desc.dispatch.workgroup_size;
    let mut violations = Vec::new();

    for (axis, &dim) in wg.iter().enumerate() {
        let axis = axis as u8;
        if dim == 0 {
            violations.push(Violation::ZeroDim { axis });
        } else if dim > limits.max_workgroup_size_per_dim[axis as usize] {
            violations.push(Violation::DimExceeded {
                axis,
                actual: dim,
                limit: limits.max_workgroup_size_per_dim[axis as usize],
            });
        }
    }

    let invocations = wg[0].saturating_mul(wg[1]).saturating_mul(wg[2]);
    if invocations > limits.max_workgroup_invocations {
        violations.push(Violation::InvocationsExceeded {
            actual: invocations,
            limit: limits.max_workgroup_invocations,
        });
    }

    ValidationReport {
        kernel_id: desc.id.clone(),
        workgroup_size: wg,
        limits,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_lower::{BindingLayout, Dispatch, KernelBody, KernelDescriptor};

    fn empty_with_dispatch(d: Dispatch) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: d,
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    #[test]
    fn small_workgroup_is_valid() {
        let report = analyze(&empty_with_dispatch(Dispatch::new(64, 1, 1)));
        assert!(report.ok());
        assert_eq!(report.invocations(), 64);
    }

    #[test]
    fn standard_1d_1024_workgroup_is_valid_at_baseline() {
        let report = analyze(&empty_with_dispatch(Dispatch::new(1024, 1, 1)));
        assert!(report.ok());
    }

    #[test]
    fn dim_x_over_1024_violates_dim_limit() {
        let report = analyze(&empty_with_dispatch(Dispatch::new(2048, 1, 1)));
        assert!(!report.ok());
        let has_dim_violation = report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::DimExceeded { axis: 0, .. }));
        assert!(has_dim_violation);
    }

    #[test]
    fn dim_z_over_64_violates_baseline() {
        let report = analyze(&empty_with_dispatch(Dispatch::new(1, 1, 128)));
        assert!(!report.ok());
        let has = report.violations.iter().any(|v| {
            matches!(
                v,
                Violation::DimExceeded {
                    axis: 2,
                    actual: 128,
                    limit: 64
                }
            )
        });
        assert!(has);
    }

    #[test]
    fn product_over_1024_violates_invocations() {
        // 32x32x2 = 2048  -  within per-dim, over invocations.
        let report = analyze(&empty_with_dispatch(Dispatch::new(32, 32, 2)));
        assert!(!report.ok());
        let has = report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::InvocationsExceeded { actual: 2048, .. }));
        assert!(has);
    }

    #[test]
    fn zero_dim_y_flagged() {
        let report = analyze(&empty_with_dispatch(Dispatch::new(64, 0, 1)));
        let has = report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::ZeroDim { axis: 1 }));
        assert!(has);
    }

    #[test]
    fn high_end_device_profile_allows_more() {
        // Custom profile: NVIDIA modern desktop allows 1024x1024x1024
        // (well above baseline z=64) and a higher product limit.
        let limits = DeviceLimits {
            max_workgroup_size_per_dim: [1024, 1024, 1024],
            max_workgroup_invocations: 1024,
        };
        // 1x1x128 should fail baseline (z>64) but pass this profile (z<1024).
        let report = analyze_against(&empty_with_dispatch(Dispatch::new(1, 1, 128)), limits);
        assert!(report.ok());
    }

    #[test]
    fn invocations_helper_computes_product() {
        let report = analyze(&empty_with_dispatch(Dispatch::new(8, 8, 4)));
        assert_eq!(report.invocations(), 256);
    }

    #[test]
    fn carries_kernel_id() {
        let mut desc = empty_with_dispatch(Dispatch::new(1, 1, 1));
        desc.id = "named".into();
        let report = analyze(&desc);
        assert_eq!(report.kernel_id, "named");
    }
}
