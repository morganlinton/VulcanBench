//! Canonical pre-emit lowering pipeline.
//!
//! This is the single production boundary from high-level `Program` IR to
//! emitter-ready `KernelDescriptor`: inline calls, run semantic Program
//! optimization, lower to descriptor form, verify, run descriptor cleanup, and
//! verify again. Backends should not assemble their own partial version of
//! this sequence.

use crate::descriptor::KernelDescriptor;
use crate::lower::lower;
use crate::rewrites::OptimizationStats;
use crate::{verify_then_optimize, VerifyFailure};
use std::fmt;
use vyre_foundation::ir::Program;

/// Program + descriptor pair produced by the canonical pre-emit pipeline.
#[derive(Debug, Clone)]
pub struct LoweredKernel {
    /// Program after call inlining and IR-semantic optimization.
    pub program: Program,
    /// Verified descriptor after descriptor-level cleanup rewrites.
    pub descriptor: KernelDescriptor,
    /// Descriptor rewrite statistics collected from the cleanup phase.
    pub descriptor_stats: OptimizationStats,
}

/// Error raised by the canonical pre-emit pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreEmitError {
    message: String,
}

impl PreEmitError {
    fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        debug_assert!(message.contains("Fix:"));
        Self { message }
    }

    /// Return the actionable diagnostic.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PreEmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PreEmitError {}

/// Inline calls and run the semantic Program-level optimizer.
///
/// This prepares high-level IR for descriptor lowering while preserving the
/// distinction between Layer-1 semantic rewrites and lowered descriptor
/// cleanup.
///
/// # Errors
///
/// Returns [`PreEmitError`] when call inlining fails.
pub fn prepare_program_for_emit(program: &Program) -> Result<Program, PreEmitError> {
    let pruned = vyre_foundation::optimizer::pre_lowering::optimize(program.clone());
    let pruned = lower_single_rank_collectives_for_emit(pruned)?;
    let inlined = vyre_foundation::ir::inline_calls(&pruned).map_err(|error| {
        PreEmitError::new(format!(
            "call inlining failed before descriptor lowering: {error}. Fix: register every Expr::Call target with the active dialect resolver or eliminate the call before backend emission."
        ))
    })?;
    lower_single_rank_collectives_for_emit(vyre_foundation::optimizer::pre_lowering::optimize(
        inlined,
    ))
}

fn lower_single_rank_collectives_for_emit(program: Program) -> Result<Program, PreEmitError> {
    match vyre_foundation::transform::collectives::lower_single_rank_collectives(&program) {
        Ok(Some(lowered)) => Ok(lowered),
        Ok(None) => Ok(program),
        Err(error) => Err(PreEmitError::new(format!(
            "single-rank collective lowering failed before descriptor lowering: {error}. Fix: route true multi-rank collectives through a backend transport path or lower them before pre-emit."
        ))),
    }
}

/// Run the complete canonical pre-emit pipeline.
///
/// # Errors
///
/// Returns [`PreEmitError`] when inlining, descriptor lowering, input
/// verification, descriptor cleanup, or output verification fails.
pub fn lower_for_emit(program: &Program) -> Result<LoweredKernel, PreEmitError> {
    let program = prepare_program_for_emit(program)?;
    let descriptor = lower(&program).map_err(|error| {
        PreEmitError::new(format!(
            "KernelDescriptor lowering failed after semantic Program optimization: {error}. Fix: add the missing neutral descriptor mapping before any concrete backend emits this Program."
        ))
    })?;
    let (descriptor, descriptor_stats) = verify_then_optimize(&descriptor).map_err(|error| {
        PreEmitError::new(format!(
            "KernelDescriptor verification/cleanup failed in the canonical pre-emit pipeline: {}. Fix: repair vyre-lower so descriptor validation succeeds before concrete emission.",
            format_verify_failure(&error)
        ))
    })?;
    Ok(LoweredKernel {
        program,
        descriptor,
        descriptor_stats,
    })
}

fn format_verify_failure(error: &VerifyFailure) -> String {
    let stage = match error {
        VerifyFailure::Input(_) => "input",
        VerifyFailure::Output(_) => "output",
    };
    let mut out = format!("{stage} descriptor invalid");
    for (index, err) in error.errors().iter().take(4).enumerate() {
        if index == 0 {
            out.push_str(": ");
        } else {
            out.push_str("; ");
        }
        out.push_str(&format!("{err:?}"));
    }
    if error.errors().len() > 4 {
        out.push_str("; ...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KernelBody, KernelOpKind};
    use vyre_foundation::ir::{
        BufferAccess, BufferDecl, CollectiveOp, CommGroup, DataType, Expr, Ident, Node,
    };

    #[test]
    fn lower_for_emit_runs_program_and_descriptor_pipeline() {
        let buffer =
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16);
        let program = Program::wrapped(
            vec![buffer],
            [64, 1, 1],
            vec![Node::Store {
                buffer: Ident::from("out"),
                index: Expr::InvocationId { axis: 0 },
                value: Expr::LitU32(7),
            }],
        );

        let lowered = lower_for_emit(&program).expect("Fix: pre-emit lowering must pass");

        assert_eq!(lowered.program.workgroup_size(), [64, 1, 1]);
        assert_eq!(lowered.descriptor.dispatch.workgroup_size, [64, 1, 1]);
        assert_eq!(lowered.descriptor.bindings.slots.len(), 1);
        assert!(crate::verify::verify(&lowered.descriptor).is_ok());
        assert!(lowered.descriptor_stats.iterations >= 1);
    }

    #[test]
    fn lower_for_emit_rejects_invalid_descriptor_before_backend_emit() {
        let program = Program::wrapped(Vec::new(), [0, 1, 1], Vec::new());

        let error = lower_for_emit(&program).expect_err("zero dispatch must fail");

        assert!(error.message().contains("KernelDescriptor"));
        assert!(error.message().contains("Fix:"));
    }

    #[test]
    fn lower_for_emit_lowers_world_allgather_before_descriptor_lowering() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(4),
                BufferDecl::output("out", 1, DataType::U32).with_count(4),
            ],
            [64, 1, 1],
            vec![Node::AllGather {
                input: "input".into(),
                output: "out".into(),
                group: CommGroup::WORLD,
            }],
        );

        let lowered = lower_for_emit(&program).expect(
            "Fix: canonical pre-emit must lower WORLD AllGather before descriptor lowering.",
        );

        assert!(!lowered.program.stats().distributed_collectives());
        assert!(crate::verify::verify(&lowered.descriptor).is_ok());
    }

    #[test]
    fn lower_for_emit_rejects_transport_collectives_before_descriptor_lowering() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(4),
                BufferDecl::output("out", 1, DataType::U32).with_count(4),
            ],
            [64, 1, 1],
            vec![Node::ReduceScatter {
                input: "input".into(),
                output: "out".into(),
                op: CollectiveOp::Sum,
                group: CommGroup(7),
            }],
        );

        let error = lower_for_emit(&program)
            .expect_err("Fix: canonical pre-emit must reject collectives that need transport.");

        assert!(error.message().contains("Multi-rank collective transport"));
    }

    #[test]
    fn concrete_backend_program_descriptor_lowering_is_single_sourced() {
        use std::fmt::Write as _;

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("Fix: vyre-lower must live under the vyre workspace root.");
        let mut files = Vec::default();
        for rel in [
            "vyre-driver/src",
            "vyre-driver-cuda/src",
            "vyre-driver-metal/src",
            "vyre-driver-wgpu/src",
            "vyre-emit-naga/src",
            "vyre-emit-ptx/src",
        ] {
            collect_rust_files(&root.join(rel), &mut files)
                .expect("Fix: boundary scan must read concrete backend source trees.");
        }

        let mut violations = Vec::default();
        for path in files {
            let text = std::fs::read_to_string(&path)
                .expect("Fix: boundary scan must read Rust source files.");
            for (line_no, line) in text.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                if trimmed.starts_with("#[cfg(test)]") {
                    break;
                }
                if line.contains("vyre_lower::lower(")
                    || line.contains("vyre_lower::prepare_program_for_emit(")
                    || line.contains("vyre_foundation::optimizer::pre_lowering::optimize(")
                    || line.contains("vyre_foundation::ir::inline_calls(")
                {
                    let mut violation = String::default();
                    let _ = write!(
                        &mut violation,
                        "{}:{}:{}",
                        path.strip_prefix(root).unwrap_or(&path).display(),
                        line_no + 1,
                        trimmed
                    );
                    violations.push(violation);
                }
            }
        }

        assert!(
            violations.is_empty(),
            "Fix: concrete backend Program input must enter through vyre_lower::lower_for_emit or consume a typed KernelDescriptor artifact, not raw lowering steps: {violations:?}"
        );
    }

    fn collect_rust_files(
        dir: &std::path::Path,
        out: &mut Vec<std::path::PathBuf>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, out)?;
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
        Ok(())
    }

    #[test]
    fn lower_for_emit_preserves_loop_carrier_swap_snapshot() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("instrs", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(7),
                BufferDecl::output("results", 1, DataType::U32).with_count(1),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("tid", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("tid"), Expr::u32(1)),
                    vec![
                        Node::let_bind("base", Expr::mul(Expr::var("tid"), Expr::u32(7))),
                        Node::let_bind("s0", Expr::u32(0)),
                        Node::let_bind("s1", Expr::u32(0)),
                        Node::let_bind("s2", Expr::u32(0)),
                        Node::let_bind("s3", Expr::u32(0)),
                        Node::Loop {
                            var: "pc".into(),
                            from: Expr::u32(0),
                            to: Expr::u32(7),
                            body: vec![
                                Node::let_bind(
                                    "instr",
                                    Expr::load(
                                        "instrs",
                                        Expr::add(Expr::var("base"), Expr::var("pc")),
                                    ),
                                ),
                                Node::let_bind(
                                    "op",
                                    Expr::bitand(Expr::var("instr"), Expr::u32(0xFF)),
                                ),
                                Node::let_bind("imm", Expr::shr(Expr::var("instr"), Expr::u32(8))),
                                Node::if_then(
                                    Expr::eq(Expr::var("op"), Expr::u32(0)),
                                    vec![
                                        Node::assign("s3", Expr::var("s2")),
                                        Node::assign("s2", Expr::var("s1")),
                                        Node::assign("s1", Expr::var("s0")),
                                        Node::assign("s0", Expr::var("imm")),
                                    ],
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("op"), Expr::u32(1)),
                                    vec![
                                        Node::assign(
                                            "s0",
                                            Expr::add(Expr::var("s0"), Expr::var("s1")),
                                        ),
                                        Node::assign("s1", Expr::var("s2")),
                                        Node::assign("s2", Expr::var("s3")),
                                        Node::assign("s3", Expr::u32(0)),
                                    ],
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("op"), Expr::u32(2)),
                                    vec![
                                        Node::assign(
                                            "s0",
                                            Expr::mul(Expr::var("s0"), Expr::var("s1")),
                                        ),
                                        Node::assign("s1", Expr::var("s2")),
                                        Node::assign("s2", Expr::var("s3")),
                                        Node::assign("s3", Expr::u32(0)),
                                    ],
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("op"), Expr::u32(3)),
                                    vec![
                                        Node::assign("s3", Expr::var("s2")),
                                        Node::assign("s2", Expr::var("s1")),
                                        Node::assign("s1", Expr::var("s0")),
                                    ],
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("op"), Expr::u32(4)),
                                    vec![
                                        Node::let_bind("tmp", Expr::var("s0")),
                                        Node::assign("s0", Expr::var("s1")),
                                        Node::assign("s1", Expr::var("tmp")),
                                    ],
                                ),
                            ],
                        },
                        Node::store("results", Expr::var("tid"), Expr::var("s0")),
                    ],
                ),
            ],
        );

        let lowered = lower_for_emit(&program).expect("Fix: pre-emit lowering must pass");

        assert!(
            body_has_s1_end_from_copy(&lowered.descriptor.body),
            "Fix: lowering must preserve `let tmp = s0` as a Copy snapshot so SWAP writes s1 from old s0 instead of the post-assign s0 carrier"
        );
    }

    fn body_has_s1_end_from_copy(body: &KernelBody) -> bool {
        body.ops.iter().any(|op| {
            let KernelOpKind::LoopCarrierEnd { name } = &op.kind else {
                return false;
            };
            name.as_ref() == "s1"
                && op.operands.first().copied().is_some_and(|operand| {
                    body.ops.iter().any(|producer| {
                        producer.result == Some(operand)
                            && matches!(producer.kind, KernelOpKind::Copy)
                    })
                })
        }) || body.child_bodies.iter().any(body_has_s1_end_from_copy)
    }
}
