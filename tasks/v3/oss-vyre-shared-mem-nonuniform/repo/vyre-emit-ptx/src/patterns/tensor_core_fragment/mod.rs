//! PERF B6: tensor-core (wmma/mma) fragment promotion candidate detection.
//!
//! Detects KernelOp groups whose shape divides the wmma fragment tile
//! (16×16×16 for f16 on sm_70+, 16×16×16 for bf16 on sm_80+). Real
//! emission of wmma fragments is phase 2; this module identifies the
//! candidates so the emit-time decision can be made.
//!
//! Phase-1 detection criteria:
//! - Workgroup-size dimensions are multiples of the fragment tile.
//! - Kernel has an FMA-chain pattern that looks like matmul accumulation
//!   (sum of products into a register).
//! - Element type is f16 / bf16 / f32 (wmma fragments require these).
//!
//! The `analyze` returns a `TensorCorePlan` listing which fragment
//! shapes are eligible for the kernel on the given target capability.

use serde::{Deserialize, Serialize};
use vyre_lower::{KernelBody, KernelDescriptor, KernelOpKind};

use crate::ComputeCapability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FragmentTile {
    /// 16×16×16 f16 fragment, supported on sm_70+.
    F16_16x16x16,
    /// 16×16×16 bf16 fragment, supported on sm_80+.
    Bf16_16x16x16,
    /// 8×8×16 f16 fragment for tiny tiles.
    F16_8x8x16,
}

impl FragmentTile {
    #[must_use]
    pub fn supported_on(&self, target: ComputeCapability) -> bool {
        match self {
            Self::F16_16x16x16 | Self::F16_8x8x16 => target.supports_wmma_f16(),
            Self::Bf16_16x16x16 => target.supports_wmma_bf16(),
        }
    }

    #[must_use]
    pub fn dims(&self) -> (u32, u32, u32) {
        match self {
            Self::F16_16x16x16 | Self::Bf16_16x16x16 => (16, 16, 16),
            Self::F16_8x8x16 => (8, 8, 16),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorCoreCandidate {
    pub fragment: FragmentTile,
    /// FMA op count in the kernel  -  the higher this is, the more
    /// accumulation work goes through tensor cores.
    pub fma_op_count: u32,
    /// Estimated speedup over scalar FMA chain. Conservative
    /// `5.0 + log2(fma_op_count)` to avoid overpromise.
    pub estimated_speedup_factor: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorCorePlan {
    pub kernel_id: String,
    pub target_sm: String,
    pub candidates: Vec<TensorCoreCandidate>,
}

impl TensorCorePlan {
    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor, target: ComputeCapability) -> TensorCorePlan {
    let fma_count = count_fma(&desc.body);
    let workgroup_aligned = workgroup_size_aligned(desc.dispatch.workgroup_size);

    let mut candidates = Vec::new();
    if fma_count >= 4 && workgroup_aligned {
        // Conservative speedup: 5.0 baseline + log2 scaling.
        let speedup = 5.0 + (fma_count as f32).log2();
        for tile in [
            FragmentTile::F16_16x16x16,
            FragmentTile::Bf16_16x16x16,
            FragmentTile::F16_8x8x16,
        ] {
            if tile.supported_on(target) {
                candidates.push(TensorCoreCandidate {
                    fragment: tile,
                    fma_op_count: fma_count,
                    estimated_speedup_factor: speedup,
                });
            }
        }
    }

    TensorCorePlan {
        kernel_id: desc.id.clone(),
        target_sm: format!("sm_{}{}", target.major, target.minor),
        candidates,
    }
}

fn count_fma(body: &KernelBody) -> u32 {
    let mut total: u32 = body
        .ops
        .iter()
        .filter(|op| matches!(op.kind, KernelOpKind::Fma))
        .count() as u32;
    for child in &body.child_bodies {
        total = total.saturating_add(count_fma(child));
    }
    total
}

fn workgroup_size_aligned(size: [u32; 3]) -> bool {
    // wmma requires workgroup_size_x ≥ 32 (warp size) and divides 16.
    size[0] >= 32 && size[0] % 16 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_lower::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, LiteralValue,
    };

    fn fma_kernel(fma_count: u32, workgroup_x: u32) -> KernelDescriptor {
        let mut ops = vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(1),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![2],
                result: Some(2),
            },
        ];
        for i in 0..fma_count {
            ops.push(KernelOp {
                kind: KernelOpKind::Fma,
                operands: vec![0, 1, 2],
                result: Some(3 + i),
            });
        }
        KernelDescriptor {
            id: "fma_chain".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(workgroup_x, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::F32(1.0),
                    LiteralValue::F32(2.0),
                    LiteralValue::F32(3.0),
                ],
            },
        }
    }

    #[test]
    fn empty_kernel_has_no_candidates() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn fma_chain_aligned_workgroup_yields_candidates_on_sm_80() {
        let desc = fma_kernel(8, 64);
        let p = analyze(&desc, ComputeCapability::SM_80);
        // sm_80 supports both F16 and BF16 fragments.
        assert_eq!(p.candidates.len(), 3); // F16_16, BF16_16, F16_8
        assert_eq!(p.target_sm, "sm_80");
    }

    #[test]
    fn fma_chain_on_sm_70_only_offers_f16_fragments() {
        let desc = fma_kernel(8, 64);
        let p = analyze(&desc, ComputeCapability::SM_70);
        // sm_70 supports F16 fragments only, not BF16.
        let bf16_count = p
            .candidates
            .iter()
            .filter(|c| matches!(c.fragment, FragmentTile::Bf16_16x16x16))
            .count();
        assert_eq!(bf16_count, 0);
        let f16_count = p
            .candidates
            .iter()
            .filter(|c| {
                matches!(
                    c.fragment,
                    FragmentTile::F16_16x16x16 | FragmentTile::F16_8x8x16
                )
            })
            .count();
        assert_eq!(f16_count, 2);
    }

    #[test]
    fn small_fma_count_below_threshold_no_candidates() {
        let desc = fma_kernel(2, 64);
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(
            p.candidates.is_empty(),
            "fewer than 4 FMAs not worth promoting"
        );
    }

    #[test]
    fn unaligned_workgroup_no_candidates() {
        let desc = fma_kernel(8, 33); // 33 doesn't divide 16
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn small_workgroup_no_candidates() {
        let desc = fma_kernel(8, 16); // <32, below warp size
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn fragment_dims_match_documented_shapes() {
        assert_eq!(FragmentTile::F16_16x16x16.dims(), (16, 16, 16));
        assert_eq!(FragmentTile::Bf16_16x16x16.dims(), (16, 16, 16));
        assert_eq!(FragmentTile::F16_8x8x16.dims(), (8, 8, 16));
    }

    #[test]
    fn f16_fragment_supported_on_sm_70_plus() {
        assert!(FragmentTile::F16_16x16x16.supported_on(ComputeCapability::SM_70));
        assert!(FragmentTile::F16_16x16x16.supported_on(ComputeCapability::SM_90));
    }

    #[test]
    fn bf16_fragment_only_on_sm_80_plus() {
        assert!(!FragmentTile::Bf16_16x16x16.supported_on(ComputeCapability::SM_70));
        assert!(!FragmentTile::Bf16_16x16x16.supported_on(ComputeCapability::SM_75));
        assert!(FragmentTile::Bf16_16x16x16.supported_on(ComputeCapability::SM_80));
    }

    #[test]
    fn speedup_grows_with_log_fma_count() {
        let desc = fma_kernel(16, 64);
        let p = analyze(&desc, ComputeCapability::SM_80);
        // 5.0 + log2(16) = 9.0
        assert!((p.candidates[0].estimated_speedup_factor - 9.0).abs() < 1e-5);
    }

    #[test]
    fn target_sm_string_formatted_correctly() {
        let desc = fma_kernel(8, 64);
        for (target, expected) in [
            (ComputeCapability::SM_70, "sm_70"),
            (ComputeCapability::SM_75, "sm_75"),
            (ComputeCapability::SM_80, "sm_80"),
            (ComputeCapability::SM_90, "sm_90"),
        ] {
            let p = analyze(&desc, target);
            assert_eq!(p.target_sm, expected);
        }
    }
}
