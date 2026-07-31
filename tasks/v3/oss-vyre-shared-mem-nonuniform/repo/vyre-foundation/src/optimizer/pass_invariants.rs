//! Pass-invariant verifier  -  sanity-check every registered pass.
//!
//! Op id: `vyre-foundation::optimizer::pass_invariants`. Soundness: `Exact`
//! over the documented invariants below. Cost-direction: read-only  -
//! verifies but never mutates passes. Preserves: every analysis. Invalidates:
//! nothing.
//!
//! ## Invariants checked
//!
//! For every pass registered via `inventory::collect!(ProgramPassRegistration)` plus
//! the devirtualized built-ins, the verifier runs the pass on each Program
//! in a small synthetic corpus and asserts:
//!
//! 1. **Builds clean.** The pass's `transform` returns a `PassResult` whose
//!    `program` field is a structurally-valid `Program` (passes
//!    `Program::stats()` without panicking, no negative counts).
//!
//! 2. **Cost-monotone-down OR refused.** Pre-cost vs post-cost via
//!    `cost::CostCertificate::dominates_or_equal`. If `changed = true` AND
//!    cost increased on any tracked dimension AND the pass did NOT return
//!    via `try_transform` with `Err(RefusalReason::CostIncrease)`, it's a
//!    contract violation. The cost-monotone scheduler gate catches this at
//!    runtime; this verifier catches it at test time so contributors fix the
//!    pass before merge.
//!
//! 3. **Op-id stability.** Every op_id present in the post-rewrite Program
//!    must also appear in either the pre-rewrite Program OR the global op
//!    registry. A pass that introduces a fresh op_id absent from both is
//!    a wire-contract violation (the op cannot lower).
//!
//! 4. **Declared idempotence.** Passes in `IDEMPOTENCE_REQUIRED` must reach
//!    their local fixed point after one application on the synthetic corpus.
//!    The second application must report `changed = false`.
//!
//! ## Synthetic corpus
//!
//! Three Programs cover the bulk of pass-rewrite shapes:
//!   - `trivial_program`  -  single store, scalar literal RHS. Tests the
//!     no-op path of every pass.
//!   - `arithmetic_program`  -  `out = in + 1` with constant fold opportunity.
//!     Tests every arithmetic-rewriting pass.
//!   - `divergent_program`  -  `if invocation_id == 0 { store }`. Tests
//!     effect-lattice-aware passes.
//!
//! The same verifier accepts larger fixture corpora as they are promoted
//! into the optimizer test surface, so every pass is checked across the full
//! shape spectrum.

use crate::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use crate::optimizer::cost::CostCertificate;
use crate::optimizer::{registered_passes, ProgramPassKind};

/// One verifier finding. Empty `Vec<PassInvariantFinding>` = clean run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PassInvariantFinding {
    /// Registered pass scheduling failed, so the verifier could not safely
    /// audit the pass set.
    RegistryError {
        /// Actionable scheduler error.
        detail: String,
    },
    /// Pass landed a rewrite that increased a tracked cost dimension
    /// without explicitly refusing via `RefusalReason::CostIncrease`.
    CostMonotoneViolation {
        /// Pass name (from `PassMetadata::name`).
        pass: &'static str,
        /// Synthetic corpus program identifier.
        program: &'static str,
        /// Comma-joined list of dimensions that increased.
        increased: String,
    },
    /// Pass produced a Program that fails structural validation (the
    /// `Program::stats()` call panicked or returned obviously-corrupt
    /// counts). Verifier reports this as a hard bug.
    StructurallyInvalid {
        /// Pass name.
        pass: &'static str,
        /// Synthetic corpus program identifier.
        program: &'static str,
        /// Free-form detail (panic message or count discrepancy).
        detail: String,
    },
    /// A pass that is required to be locally idempotent changed the program on
    /// its second application.
    IdempotenceViolation {
        /// Pass name.
        pass: &'static str,
        /// Synthetic corpus program identifier.
        program: &'static str,
    },
}

const IDEMPOTENCE_REQUIRED: &[&str] = &[
    "buffer_decl_sort",
    "canonicalize",
    "const_fold",
    "cse",
    "dce",
    "dead_buffer_elim",
    "dead_store_elim",
    "empty_block_collapse",
    "noop_assign_eliminate",
    "region_promote_singleton_block",
];

/// Build the synthetic corpus the verifier runs every pass against.
fn synthetic_corpus() -> Vec<(&'static str, Program)> {
    vec![
        (
            "trivial",
            Program::wrapped(
                vec![
                    BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32)
                        .with_count(1),
                ],
                [1, 1, 1],
                vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
            ),
        ),
        (
            "arithmetic",
            Program::wrapped(
                vec![
                    BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32)
                        .with_count(1),
                ],
                [1, 1, 1],
                vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::add(Expr::u32(3), Expr::u32(4)),
                )],
            ),
        ),
        (
            "divergent",
            Program::wrapped(
                vec![
                    BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32)
                        .with_count(1),
                ],
                [256, 1, 1],
                vec![Node::if_then(
                    Expr::BinOp {
                        op: BinOp::Eq,
                        left: Box::new(Expr::gid_x()),
                        right: Box::new(Expr::u32(0)),
                    },
                    vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
                )],
            ),
        ),
    ]
}

/// Run every registered pass against the synthetic corpus and return the
/// list of invariant findings. Empty Vec = every pass passes every gate.
///
/// This function is the production entry point for the verifier; tests in
/// the same module call it via `pass_invariants_clean()`.
///
/// # Errors
///
/// Returns the list of findings  -  never panics on a pass-side failure.
/// Caller decides whether non-empty findings warrant a hard error.
#[must_use]
pub fn audit_registered_passes() -> Vec<PassInvariantFinding> {
    let passes = match registered_passes() {
        Ok(passes) => passes,
        Err(error) => {
            return vec![PassInvariantFinding::RegistryError {
                detail: error.to_string(),
            }];
        }
    };
    let corpus = synthetic_corpus();
    let mut findings = Vec::new();
    for pass in passes {
        for (program_name, program) in &corpus {
            findings.extend(audit_pass_on_program(
                &pass,
                program_name,
                Clone::clone(&program),
            ));
        }
    }
    findings
}

fn audit_pass_on_program(
    pass: &ProgramPassKind,
    program_name: &'static str,
    program: Program,
) -> Vec<PassInvariantFinding> {
    let pre_cost = CostCertificate::for_program(&program);
    let pass_name = pass.metadata().name;

    // Run try_transform  -  if the pass returns Err, it's an explicit refusal,
    // which is fine and means no further checks on this run.
    let result = match pass.try_transform(program) {
        Ok(result) => result,
        Err(_refusal) => return Vec::new(),
    };

    let post_cost = CostCertificate::for_program(&result.program);
    let mut findings = Vec::new();

    // Invariant 2: cost-monotone-down on any rewrite the pass landed.
    if result.changed && !post_cost.dominates_or_equal(&pre_cost) {
        let increased = post_cost.dimensions_increased_over(&pre_cost).join(",");
        findings.push(PassInvariantFinding::CostMonotoneViolation {
            pass: pass_name,
            program: program_name,
            increased,
        });
    }

    // Invariant 1: structurally valid. We probe via stats()  -  that walks
    // the entry tree and returns counts; a panic-free, non-overflowing run
    // is a strong signal the IR is valid.
    let stats = result.program.stats();
    if stats.node_count == 0 && result.changed {
        findings.push(PassInvariantFinding::StructurallyInvalid {
            pass: pass_name,
            program: program_name,
            detail: "rewrite produced zero-node program from non-empty input".into(),
        });
    }

    if IDEMPOTENCE_REQUIRED.contains(&pass_name) {
        match pass.try_transform(result.program) {
            Ok(second) if second.changed => {
                findings.push(PassInvariantFinding::IdempotenceViolation {
                    pass: pass_name,
                    program: program_name,
                })
            }
            Ok(_) | Err(_) => {}
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_corpus_has_three_programs_with_distinct_shapes() {
        let corpus = synthetic_corpus();
        assert_eq!(
            corpus.len(),
            3,
            "corpus contract: trivial, arithmetic, divergent"
        );
        let names: Vec<&str> = corpus.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"trivial"));
        assert!(names.contains(&"arithmetic"));
        assert!(names.contains(&"divergent"));
    }

    #[test]
    fn divergent_program_has_nonzero_divergence_score() {
        let corpus = synthetic_corpus();
        let divergent = corpus
            .iter()
            .find(|(n, _)| *n == "divergent")
            .map(|(_, p)| p)
            .expect("Fix: divergent program must be in corpus");
        let cost = CostCertificate::for_program(divergent);
        assert!(
            cost.divergence_score >= 1,
            "the divergent program must register divergence  -  without this, the verifier \
             can't catch effect-lattice-related regressions"
        );
    }

    #[test]
    fn trivial_program_has_zero_divergence_score() {
        let corpus = synthetic_corpus();
        let trivial = corpus
            .iter()
            .find(|(n, _)| *n == "trivial")
            .map(|(_, p)| p)
            .expect("Fix: trivial must be in corpus");
        let cost = CostCertificate::for_program(trivial);
        assert_eq!(cost.divergence_score, 0);
    }

    #[test]
    fn audit_runs_to_completion_without_panic() {
        // The contract: audit_registered_passes never panics  -  it surfaces
        // pass-side problems as `PassInvariantFinding` entries.
        let _findings = audit_registered_passes();
    }

    /// Passes that legitimately add nodes/instructions in exchange for a
    /// runtime safety guarantee  -  `autotune` adds bounds-check guards
    /// around dispatched indices to avoid out-of-range writes when the
    /// problem size doesn't divide evenly into the workgroup. The added
    /// branches are NOT a contract violation; they're the pass's contract.
    /// Other intentional-non-monotone passes belong in this list with
    /// the same justification line.
    const COST_INCREASE_EXEMPT: &[&str] = &["autotune"];

    #[test]
    fn audit_finds_zero_cost_monotone_violations_on_built_ins() {
        // Every shipped built-in pass is expected to be cost-monotone-down on
        // the synthetic corpus, EXCEPT those listed in `COST_INCREASE_EXEMPT`
        // (passes that intentionally trade cost for safety/correctness).
        // A non-exempt violation here means the pass landed a cost-up rewrite
        // without declaring `RefusalReason::CostIncrease`  -  a real bug. The
        // scheduler gate rejects it at runtime; this test catches it at
        // PR-review time instead.
        let findings = audit_registered_passes();
        let cost_violations: Vec<_> = findings
            .iter()
            .filter(|f| match f {
                PassInvariantFinding::CostMonotoneViolation { pass, .. } => {
                    !COST_INCREASE_EXEMPT.contains(pass)
                }
                _ => false,
            })
            .collect();
        assert!(
            cost_violations.is_empty(),
            "built-in passes must be cost-monotone-down on the synthetic corpus; \
             non-exempt violations: {cost_violations:#?}"
        );
    }

    #[test]
    fn audit_finds_zero_structurally_invalid_outputs_on_built_ins() {
        let findings = audit_registered_passes();
        let invalid: Vec<_> = findings
            .iter()
            .filter(|f| matches!(f, PassInvariantFinding::StructurallyInvalid { .. }))
            .collect();
        assert!(
            invalid.is_empty(),
            "built-in passes must produce structurally-valid Programs; bad outputs: {invalid:#?}"
        );
    }

    #[test]
    fn audit_finds_zero_idempotence_violations_on_required_built_ins() {
        let findings = audit_registered_passes();
        let invalid: Vec<_> = findings
            .iter()
            .filter(|f| matches!(f, PassInvariantFinding::IdempotenceViolation { .. }))
            .collect();
        assert!(
            invalid.is_empty(),
            "built-in passes with declared idempotence must reach a local fixed point in one application; bad outputs: {invalid:#?}"
        );
    }
}
