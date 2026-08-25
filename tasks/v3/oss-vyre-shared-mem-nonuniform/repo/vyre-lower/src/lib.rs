#![allow(
    clippy::doc_lazy_continuation,
    clippy::double_must_use,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::redundant_closure
)]
//! Substrate-neutral lowering for vyre.
//!
//! Source-of-truth: `SEPARATION_AUDIT_2026-05-01.md` section S3.
//!
//! Backend drivers used to own lowering, emission, and dispatch in one
//! crate. That made common lowering unshareable, let emit patterns drift,
//! and left substrate-aware-but-driver-agnostic optimizations without a
//! home between lower and emit.
//!
//! This crate creates the boundary:
//!
//! ```text
//! vyre-foundation Program
//!         ↓ lower(program)
//! KernelDescriptor (this crate's pub type)
//!         ↓
//! emit crate
//!         ↓
//! backend artifact
//!         ↓
//! driver dispatch
//! ```
//!
//! `KernelDescriptor` is the substrate-neutral kernel intermediate
//! representation  -  binding layout, dispatch shape, lowered kernel
//! body. NOT the same as `vyre_foundation::Program` (which is the
//! pre-lowered IR with high-level constructs like `Node::Region`).
//! Drivers stay thin: take a backend artifact + bind buffers + dispatch.

pub mod analyses;
pub mod audit;
pub mod descriptor;
pub mod emit_adversarial_corpus;
pub mod error;
pub mod lower;
pub(crate) mod op_properties;
pub(crate) mod operand_semantics;
pub mod optimization_corpus;
pub mod pre_emit;
pub mod rewrites;
pub mod verify;

pub use audit::{
    audit, audit_optimized, audit_with_histogram, PerfAuditReport, Recommendation,
    RecommendationCategory,
};

/// Full-power entry point: verify the input descriptor, run the
/// optimization pipeline, verify the optimized output. Returns the
/// optimized descriptor + stats on success; on failure returns
/// whichever verify step failed first.
///
/// `emit_optimized` in the emit crates only `debug_assert!`s the
/// output. This entry point promotes both checks to errors that
/// production callers can route.
pub fn verify_then_optimize(
    desc: &KernelDescriptor,
) -> Result<(KernelDescriptor, rewrites::OptimizationStats), VerifyFailure> {
    if let Err(errs) = verify::verify(desc) {
        return Err(VerifyFailure::Input(errs));
    }
    let (optimized, stats) = rewrites::run_all_with_stats(desc);
    if let Err(errs) = verify::verify(&optimized) {
        return Err(VerifyFailure::Output(errs));
    }
    Ok((optimized, stats))
}

/// Which verify step failed in [`verify_then_optimize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyFailure {
    /// Input descriptor was invalid before any rewrites ran.
    Input(Vec<verify::VerifyError>),
    /// The rewrite pipeline produced an invalid descriptor  -  a real
    /// bug in the rewrite stack. The fuzz harness gates this; if you
    /// hit it in production, it's a bug to file.
    Output(Vec<verify::VerifyError>),
}

impl VerifyFailure {
    pub fn errors(&self) -> &[verify::VerifyError] {
        match self {
            VerifyFailure::Input(e) | VerifyFailure::Output(e) => e,
        }
    }
}

/// Single-call diagnostic: runs every analysis vyre-lower offers
/// (summary, histogram, perf audit, verify, optimization stats from
/// the standard pipeline) and bundles them into a single report.
/// Useful for tooling that wants a complete picture without N
/// separate function calls.
#[must_use]
pub fn full_report(desc: &KernelDescriptor) -> FullReport {
    let descriptor_id = desc.id.clone();
    let summary = desc.summary();
    let histogram = analyses::op_histogram::analyze(desc);
    let perf = audit::audit(desc);
    let verify_input = verify::verify(desc);
    let (optimized_summary, verify_output, stats, optimization_ran) = if verify_input.is_ok() {
        let (optimized, stats) = rewrites::run_all_with_stats(desc);
        let verify_output = verify::verify(&optimized);
        (optimized.summary(), verify_output, stats, true)
    } else {
        (
            summary.clone(),
            Ok(()),
            optimization_skipped_stats(desc),
            false,
        )
    };
    let fix_text = build_full_report_fix_text(
        &verify_input,
        optimization_ran.then_some(&verify_output),
    );
    FullReport {
        descriptor_id,
        summary,
        optimized_summary,
        histogram,
        perf,
        verify_input,
        verify_output,
        stats,
        optimization_ran,
        fix_text,
    }
}

/// Bundle returned by [`full_report`]. Five orthogonal views into
/// the descriptor + standard pipeline output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FullReport {
    #[serde(default)]
    pub descriptor_id: String,
    pub summary: String,
    pub optimized_summary: String,
    pub histogram: analyses::op_histogram::OpHistogram,
    pub perf: PerfAuditReport,
    pub verify_input: verify::VerifyResult,
    #[serde(default = "default_verify_result")]
    pub verify_output: verify::VerifyResult,
    pub stats: rewrites::OptimizationStats,
    #[serde(default)]
    pub optimization_ran: bool,
    #[serde(default)]
    pub fix_text: String,
}

impl FullReport {
    #[must_use]
    pub fn verify_input_status(&self) -> &'static str {
        verification_status(&self.verify_input)
    }

    #[must_use]
    pub fn verify_output_status(&self) -> &'static str {
        if self.optimization_ran {
            verification_status(&self.verify_output)
        } else {
            "SKIPPED"
        }
    }

    /// One-line headline drawn from the underlying parts. Useful for
    /// log lines.
    pub fn format_short(&self) -> String {
        format!(
            "{} | id {} | {} | {} | {} | input verify {} | output verify {}",
            self.summary,
            self.descriptor_id,
            self.histogram.format_short(),
            self.perf.format_short(),
            self.stats.format_short(),
            self.verify_input_status(),
            self.verify_output_status(),
        )
    }

    /// Multi-line human-readable view, suitable for `--verbose` CLI
    /// output. Each section has a header and is indented for readability.
    pub fn format_long(&self) -> String {
        let mut out = String::new();
        use std::fmt::Write as _;
        let _ = writeln!(out, "Kernel:");
        let _ = writeln!(out, "  descriptor id: {}", self.descriptor_id);
        let _ = writeln!(out, "  raw:       {}", self.summary);
        let _ = writeln!(out, "  optimized: {}", self.optimized_summary);
        let _ = writeln!(out, "Histogram:");
        let _ = writeln!(out, "  {}", self.histogram.format_short());
        if let Some((cat, n)) = self.histogram.dominant() {
            let _ = writeln!(out, "  dominant: {cat} ({n})");
        }
        let _ = writeln!(out, "Perf audit:");
        let _ = writeln!(out, "  {}", self.perf.format_short());
        for r in &self.perf.recommendations {
            let _ = writeln!(
                out,
                "  - [p{}] {:?}: {} (≤{:.2}× speedup)",
                r.priority, r.category, r.message, r.estimated_speedup_upper_bound
            );
        }
        let _ = writeln!(out, "Optimization:");
        let _ = writeln!(out, "  {}", self.stats.format_short());
        let _ = writeln!(out, "Verify (input):");
        write_verify_section(&mut out, &self.verify_input);
        let _ = writeln!(out, "Verify (output):");
        if self.optimization_ran {
            write_verify_section(&mut out, &self.verify_output);
        } else {
            let _ = writeln!(out, "  SKIPPED (input verification failed)");
        }
        if !self.fix_text.is_empty() {
            let _ = writeln!(out, "Fix:");
            let _ = writeln!(out, "  {}", self.fix_text);
        }
        out
    }
}

fn default_verify_result() -> verify::VerifyResult {
    Ok(())
}

fn optimization_skipped_stats(desc: &KernelDescriptor) -> rewrites::OptimizationStats {
    rewrites::OptimizationStats {
        ops_before: desc.body.ops.len(),
        ops_after: desc.body.ops.len(),
        bindings_before: desc.bindings.slots.len(),
        bindings_after: desc.bindings.slots.len(),
        literals_before: desc.body.literals.len(),
        literals_after: desc.body.literals.len(),
        iterations: 0,
        converged: false,
    }
}

fn verification_status(result: &verify::VerifyResult) -> &'static str {
    if result.is_ok() {
        "OK"
    } else {
        "FAIL"
    }
}

fn write_verify_section(out: &mut String, result: &verify::VerifyResult) {
    use std::fmt::Write as _;
    match result {
        Ok(()) => {
            let _ = writeln!(out, "  OK");
        }
        Err(errs) => {
            let _ = writeln!(out, "  FAIL ({} errors)", errs.len());
            for e in errs {
                let _ = writeln!(out, "    {:?}", e);
            }
        }
    }
}

fn build_full_report_fix_text(
    verify_input: &verify::VerifyResult,
    verify_output: Option<&verify::VerifyResult>,
) -> String {
    let mut messages = Vec::new();
    push_verify_fix_text("input", verify_input, &mut messages);
    if let Some(verify_output) = verify_output {
        push_verify_fix_text("output", verify_output, &mut messages);
    }
    messages.join(" ")
}

fn push_verify_fix_text(
    stage: &str,
    result: &verify::VerifyResult,
    messages: &mut Vec<String>,
) {
    if let Err(errs) = result {
        if errs.is_empty() {
            messages.push(format!(
                "Fix: {stage} descriptor verification returned an empty error list; treat this as a verifier contract bug and preserve the descriptor for triage."
            ));
        } else {
            messages.push(format!(
                "Fix: {stage} descriptor verification failed with {} error(s); repair the descriptor before emitting benchmark evidence. First error: {:?}",
                errs.len(),
                errs[0]
            ));
        }
    }
}

impl std::fmt::Display for FullReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_short())
    }
}
pub use verify::{verify, VerifyError, VerifyErrorKind, VerifyResult};

pub use descriptor::{
    BindingLayout, BindingSlot, BindingVisibility, DescriptorIntent, DescriptorIntentError,
    DescriptorIntentEvidence, DescriptorIntentKind, DescriptorIntentSet,
    DescriptorIntentStrategy, Dispatch, IntentAnnotatedDescriptor, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MatrixMmaElement,
    MatrixMmaLayout, MatrixMmaShape, MemoryClass, OpaqueExprData, OpaqueNodeData,
    scan_construct_intent_mapping, ScanConstructIntentClass, ScanConstructIntentMapping,
    DESCRIPTOR_INTENT_SCHEMA_VERSION, SCAN_CONSTRUCT_INTENT_MAPPINGS, TRAP_SIDECAR_NAME,
    TRAP_SIDECAR_WORDS,
};
pub use error::LowerError;
/// Re-exported so consumers matching/constructing `KernelOpKind::SubgroupReduce`
/// can name the reduction operator without depending on `vyre-foundation`.
pub use vyre_foundation::ir::SubgroupReduceOp;
pub use lower::lower;
pub use pre_emit::{lower_for_emit, prepare_program_for_emit, LoweredKernel, PreEmitError};

#[cfg(test)]
mod verify_then_optimize_tests {
    use super::*;

    #[test]
    fn valid_input_returns_optimized_and_stats() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let (out, stats) = verify_then_optimize(&desc).unwrap();
        assert_eq!(out.id, "k");
        assert!(stats.iterations >= 1);
    }

    #[test]
    fn invalid_input_returns_input_failure() {
        // Descriptor with zero workgroup_size dim  -  caught by verify.
        let desc = KernelDescriptor {
            id: "bad".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(0, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = verify_then_optimize(&desc);
        assert!(matches!(r, Err(VerifyFailure::Input(_))));
    }

    #[test]
    fn full_report_runs_every_layer_without_panic() {
        let desc = KernelDescriptor {
            id: "fr".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let report = full_report(&desc);
        assert_eq!(report.descriptor_id, "fr");
        assert!(report.summary.contains("fr:"));
        assert_eq!(report.histogram.literal, 2);
        assert_eq!(report.perf.kernel_id, "fr");
        assert!(report.verify_input.is_ok());
        assert!(report.verify_output.is_ok());
        assert!(report.optimization_ran);
        assert_eq!(report.verify_input_status(), "OK");
        assert_eq!(report.verify_output_status(), "OK");
        assert!(report.stats.iterations >= 1);
        assert!(report.fix_text.is_empty());
        // Display delegates to format_short.
        let s = format!("{report}");
        assert!(s.contains("fr:"));
        assert!(s.contains("id fr"));
        assert!(s.contains("OK"));
    }

    #[test]
    fn full_report_serializes_to_json() {
        let desc = KernelDescriptor {
            id: "fr".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let report = full_report(&desc);
        assert_eq!(report.descriptor_id, "fr");
        let json = serde_json::to_string(&report).expect("Fix: serialize");
        assert!(json.contains("\"descriptor_id\""));
        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"histogram\""));
        assert!(json.contains("\"perf\""));
        assert!(json.contains("\"verify_input\""));
        assert!(json.contains("\"verify_output\""));
        assert!(json.contains("\"stats\""));
        assert!(json.contains("\"optimization_ran\""));
        assert!(json.contains("\"fix_text\""));

        // Round-trip back through Deserialize.
        let _back: FullReport = serde_json::from_str(&json).expect("Fix: round-trip");
    }

    #[test]
    fn full_report_format_long_includes_all_sections() {
        let desc = KernelDescriptor {
            id: "fr".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let r = full_report(&desc);
        let long = r.format_long();
        assert!(long.contains("Kernel:"));
        assert!(long.contains("descriptor id: fr"));
        assert!(long.contains("Histogram:"));
        assert!(long.contains("Perf audit:"));
        assert!(long.contains("Optimization:"));
        assert!(long.contains("Verify (input):"));
        assert!(long.contains("Verify (output):"));
        assert!(long.contains("OK"));
    }

    #[test]
    fn full_report_records_verify_fix_text_for_bad_descriptor() {
        let desc = KernelDescriptor {
            id: "bad".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(0, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let report = full_report(&desc);
        assert_eq!(report.descriptor_id, "bad");
        assert_eq!(report.verify_input_status(), "FAIL");
        assert_eq!(report.verify_output_status(), "SKIPPED");
        assert!(!report.optimization_ran);
        assert_eq!(report.stats.iterations, 0);
        assert!(
            report
                .fix_text
                .contains("Fix: input descriptor verification failed"),
            "Fix: invalid descriptor reports must carry operator-actionable verifier repair text."
        );
        let long = report.format_long();
        assert!(long.contains("Verify (input):"));
        assert!(long.contains("Verify (output):"));
        assert!(long.contains("Fix:"));
    }

    #[test]
    fn errors_accessor_yields_underlying() {
        let desc = KernelDescriptor {
            id: "bad".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(0, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let f = verify_then_optimize(&desc).unwrap_err();
        assert_ne!(f.errors().len(), 0);
    }
}
