//! Program → required-capability analysis.
//!
//! Scan a `Program` and report the hardware capabilities its lowering will
//! need. Callers (backends, conformance harnesses, certificate emitters)
//! compare the required set against what a backend advertises and surface
//! `MissingCapability` *before* handing the kernel to the device, avoiding
//! panics inside `create_shader_module` / `createComputePipeline`.
//!
//! The scanner is strictly syntactic: it walks every `Expr` and `Node` in
//! the program and checks the IR surface. It intentionally does **not**
//! know anything about backend-specific lowering rules  -  that would make it
//! a circular dependency of the very thing it is supposed to gate.

use std::fmt;

use crate::ir::Program;

/// Capabilities a `Program` needs from whichever backend executes it.
///
/// This is a structured replacement for hardcoded "exempt op" lists. A
/// universal diff harness asks `scan(program)` which bits the program
/// needs, asks the backend which bits it advertises, and skips the pair
/// when they disagree. The result reasons are attached for telemetry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct RequiredCapabilities {
    /// The program invokes `Expr::SubgroupAdd`, `SubgroupBallot`, or
    /// `SubgroupShuffle`. Lowering paths need the SUBGROUP / wave-op
    /// feature on the target device.
    pub subgroup_ops: bool,
    /// The program uses any IEEE 754 binary16 operand.
    pub f16: bool,
    /// The program uses any bfloat16 operand.
    pub bf16: bool,
    /// The program uses 64-bit floats.
    pub f64: bool,
    /// The program dispatches async DMA (`Node::AsyncLoad` / `AsyncStore`).
    pub async_dispatch: bool,
    /// The program emits `Node::IndirectDispatch`.
    pub indirect_dispatch: bool,
    /// The program reaches into tensor / tensor-core operand types.
    pub tensor_ops: bool,
    /// The program uses a `Node::Trap`  -  backend needs trap propagation.
    pub trap: bool,
    /// The program uses collective communication nodes that require transport.
    pub distributed_collectives: bool,
    /// Count of collective nodes that can lower to local single-rank IR.
    pub local_single_rank_collectives: usize,
    /// Count of collective nodes that require real multi-rank transport.
    pub transport_collectives: usize,
    /// Maximum workgroup size declared by the program across all axes.
    pub max_workgroup_size: [u32; 3],
    /// Sum of `BufferDecl::count * sizeof(DataType)` across every buffer
    /// whose size can be computed statically. `0` means every buffer has
    /// dynamic size.
    pub static_storage_bytes: u64,
}

impl RequiredCapabilities {
    /// Empty set  -  the Program needs nothing beyond the minimum substrate.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Build the union of two capability sets (field-wise `OR` and `max`).
    #[must_use]
    pub fn union(mut self, other: RequiredCapabilities) -> Self {
        self.subgroup_ops |= other.subgroup_ops;
        self.f16 |= other.f16;
        self.bf16 |= other.bf16;
        self.f64 |= other.f64;
        self.async_dispatch |= other.async_dispatch;
        self.indirect_dispatch |= other.indirect_dispatch;
        self.tensor_ops |= other.tensor_ops;
        self.trap |= other.trap;
        self.distributed_collectives |= other.distributed_collectives;
        self.local_single_rank_collectives = self
            .local_single_rank_collectives
            .saturating_add(other.local_single_rank_collectives);
        self.transport_collectives = self
            .transport_collectives
            .saturating_add(other.transport_collectives);
        for axis in 0..3 {
            self.max_workgroup_size[axis] =
                self.max_workgroup_size[axis].max(other.max_workgroup_size[axis]);
        }
        self.static_storage_bytes = self
            .static_storage_bytes
            .saturating_add(other.static_storage_bytes);
        self
    }
}

/// The reason a backend cannot execute a program.
///
/// Returned by [`check_backend_capabilities`] when the scan finds a
/// capability the backend did not advertise. Carries every missing bit
/// so callers can emit one actionable error instead of bisecting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingCapability {
    /// Backend identifier that was asked to run the program.
    pub backend: String,
    /// Flat list of human-readable capability names the backend lacks.
    /// Workgroup-axis violations include the stable `"workgroup_size"`
    /// category plus `"workgroup_size axis N (requested R, max M)"`
    /// detail so callers can both match the category and point at the
    /// specific axis.
    pub missing: Vec<String>,
}

impl fmt::Display for MissingCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "backend `{}` is missing required capabilities: {}. \
             Fix: pick a GPU backend that advertises these capabilities \
             or lower the program requirements before dispatch.",
            self.backend,
            self.missing.join(", ")
        )
    }
}

impl std::error::Error for MissingCapability {}

/// Walk the program and collect the union of capabilities it requires.
#[must_use]
pub fn scan(program: &Program) -> RequiredCapabilities {
    let stats = program.stats();
    let collective_plan = crate::transform::collectives::collective_transport_plan(program);
    RequiredCapabilities {
        subgroup_ops: stats.subgroup_ops(),
        f16: stats.f16(),
        bf16: stats.bf16(),
        f64: stats.f64(),
        async_dispatch: stats.async_dispatch(),
        indirect_dispatch: stats.indirect_dispatch(),
        tensor_ops: stats.tensor_ops(),
        trap: stats.trap(),
        distributed_collectives: collective_plan.requires_transport(),
        local_single_rank_collectives: collective_plan.local_single_rank_collectives(),
        transport_collectives: collective_plan.transport_collectives(),
        max_workgroup_size: program.workgroup_size,
        static_storage_bytes: stats.static_storage_bytes,
    }
}

/// Return `Ok(())` when a backend with the given advertised capabilities
/// can run a program whose required set is `required`, otherwise return
/// the missing-capability explanation.
///
/// The caller passes in the boolean capability queries from
/// [`crate::ir::Program`]'s backend trait (`supports_subgroup_ops`,
/// `supports_f16`, etc.) so this function stays free of the
/// `VyreBackend` trait import and can live in vyre-foundation.
pub fn check_backend_capabilities(
    backend_id: &str,
    supports_subgroup_ops: bool,
    supports_half_precision: bool,
    supports_brain_float: bool,
    supports_indirect_dispatch: bool,
    supports_trap_propagation: bool,
    supports_distributed_collectives: bool,
    max_workgroup_size: [u32; 3],
    required: &RequiredCapabilities,
) -> Result<(), MissingCapability> {
    let mut missing: Vec<String> = Vec::new();
    if required.subgroup_ops && !supports_subgroup_ops {
        missing.push("subgroup_ops".to_string());
    }
    if required.f16 && !supports_half_precision {
        missing.push("f16".to_string());
    }
    if required.bf16 && !supports_brain_float {
        missing.push("bf16".to_string());
    }
    if required.indirect_dispatch && !supports_indirect_dispatch {
        missing.push("indirect_dispatch".to_string());
    }
    if required.trap && !supports_trap_propagation {
        missing.push("trap_propagation".to_string());
    }
    if required.distributed_collectives && !supports_distributed_collectives {
        missing.push("distributed_collectives".to_string());
        missing.push(format!(
            "distributed_collectives transport_collectives={} local_single_rank_collectives={}",
            required.transport_collectives, required.local_single_rank_collectives
        ));
    }
    for (axis, (req_size, max_size)) in required
        .max_workgroup_size
        .iter()
        .zip(max_workgroup_size.iter())
        .enumerate()
    {
        if *req_size > *max_size && *max_size != 0 {
            if !missing.iter().any(|item| item == "workgroup_size") {
                missing.push("workgroup_size".to_string());
            }
            missing.push(format!(
                "workgroup_size axis {axis} (requested {req_size}, max {max_size})"
            ));
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(MissingCapability {
            backend: backend_id.to_string(),
            missing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    use crate::ir::{
        BufferAccess, BufferDecl, CollectiveOp, CommGroup, DataType, Expr as IrExpr,
        Node as IrNode, Program,
    };

    fn empty_program() -> Program {
        Program::wrapped(
            vec![BufferDecl::storage(
                "out",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![IrNode::let_bind("x", IrExpr::u32(0))],
        )
    }

    #[test]
    fn scan_scalar_program_declares_no_capabilities() {
        let caps = scan(&empty_program());
        assert!(!caps.subgroup_ops);
        assert!(!caps.f16);
        assert!(!caps.async_dispatch);
        assert_eq!(caps.local_single_rank_collectives, 0);
        assert_eq!(caps.transport_collectives, 0);
    }

    #[test]
    fn scan_mixed_collectives_preserves_transport_shape() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(16),
                BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(16),
            ],
            [64, 1, 1],
            vec![IrNode::Block(vec![
                IrNode::AllGather {
                    input: "input".into(),
                    output: "out".into(),
                    group: CommGroup::WORLD,
                },
                IrNode::Broadcast {
                    buffer: "out".into(),
                    root: 3,
                    group: CommGroup::WORLD,
                },
                IrNode::ReduceScatter {
                    input: "input".into(),
                    output: "out".into(),
                    op: CollectiveOp::Sum,
                    group: CommGroup(9),
                },
            ])],
        );

        let caps = scan(&program);

        assert!(caps.distributed_collectives);
        assert_eq!(caps.local_single_rank_collectives, 1);
        assert_eq!(caps.transport_collectives, 2);
    }

    #[test]
    fn missing_collective_capability_reports_transport_shape() {
        let mut required = RequiredCapabilities::none();
        required.distributed_collectives = true;
        required.local_single_rank_collectives = 5;
        required.transport_collectives = 8;

        let error = check_backend_capabilities(
            "test-backend",
            true,
            true,
            true,
            true,
            true,
            false,
            [64, 64, 64],
            &required,
        )
        .expect_err("Fix: backend without collective transport must fail capability checks.");

        assert!(error
            .missing
            .iter()
            .any(|item| item == "distributed_collectives"));
        assert!(error
            .missing
            .iter()
            .any(|item| item.contains("transport_collectives=8")));
        assert!(error
            .missing
            .iter()
            .any(|item| item.contains("local_single_rank_collectives=5")));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2048))]

        #[test]
        fn scan_collective_counts_match_generated_transport_shape(
            local_count in 0usize..32,
            transport_count in 0usize..32,
        ) {
            let mut nodes = Vec::with_capacity(local_count + transport_count);
            for _ in 0..local_count {
                nodes.push(IrNode::AllGather {
                    input: "input".into(),
                    output: "out".into(),
                    group: CommGroup::WORLD,
                });
            }
            for root in 1..=transport_count {
                nodes.push(IrNode::Broadcast {
                    buffer: "out".into(),
                    root: root as u32,
                    group: CommGroup::WORLD,
                });
            }
            let program = Program::wrapped(
                vec![
                    BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                        .with_count(16),
                    BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32)
                        .with_count(16),
                ],
                [64, 1, 1],
                nodes,
            );

            let caps = scan(&program);

            prop_assert_eq!(caps.local_single_rank_collectives, local_count);
            prop_assert_eq!(caps.transport_collectives, transport_count);
            prop_assert_eq!(caps.distributed_collectives, transport_count != 0);
        }
    }

    #[test]
    fn scan_subgroup_add_requires_subgroup_ops() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "out",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![IrNode::let_bind(
                "s",
                IrExpr::subgroup_add(IrExpr::u32(1)),
            )],
        );
        let caps = scan(&program);
        assert!(caps.subgroup_ops);
    }

    #[test]
    fn scan_call_to_subgroup_intrinsic_requires_subgroup_ops() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "out",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![IrNode::let_bind(
                "s",
                IrExpr::call(
                    "vyre-intrinsics::math::subgroup_inclusive_add",
                    vec![IrExpr::u32(1)],
                ),
            )],
        );
        let caps = scan(&program);
        assert!(caps.subgroup_ops);
    }

    #[test]
    fn check_backend_reports_every_missing_bit() {
        let required = RequiredCapabilities {
            subgroup_ops: true,
            f16: true,
            trap: true,
            ..RequiredCapabilities::default()
        };
        let error = check_backend_capabilities(
            "test_backend",
            false,
            false,
            false,
            false,
            false,
            false,
            [64, 1, 1],
            &required,
        )
        .unwrap_err();
        assert_eq!(error.backend, "test_backend");
        assert!(error.missing.iter().any(|s| s == "subgroup_ops"));
        assert!(error.missing.iter().any(|s| s == "f16"));
        assert!(error.missing.iter().any(|s| s == "trap_propagation"));
    }

    #[test]
    fn scan_world_single_rank_collective_does_not_require_transport() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(4),
                BufferDecl::output("out", 1, DataType::U32).with_count(4),
            ],
            [64, 1, 1],
            vec![IrNode::AllGather {
                input: "input".into(),
                output: "out".into(),
                group: crate::ir::CommGroup::WORLD,
            }],
        );

        let caps = scan(&program);

        assert!(!caps.distributed_collectives);
    }

    #[test]
    fn scan_nonzero_world_broadcast_requires_transport() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
            [64, 1, 1],
            vec![IrNode::Broadcast {
                buffer: "out".into(),
                root: 1,
                group: crate::ir::CommGroup::WORLD,
            }],
        );

        let caps = scan(&program);

        assert!(caps.distributed_collectives);
    }
}
