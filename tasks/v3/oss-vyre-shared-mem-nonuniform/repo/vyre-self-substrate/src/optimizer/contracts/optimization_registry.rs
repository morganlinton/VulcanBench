//! Typed optimization registry for platform and consumer release work.

use std::collections::{BTreeMap, BTreeSet};

/// Discoverable optimization pass contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptimizationPass {
    /// Stable pass identifier.
    pub id: &'static str,
    /// Owning subsystem.
    pub owner: &'static str,
    /// Pipeline phase.
    pub phase: &'static str,
    /// Correctness invariant preserved by the pass.
    pub invariant: &'static str,
    /// Benchmark or test gate proving the pass.
    pub benchmark: &'static str,
}

/// Stable explanation emitted when an optimization pass fires.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizationPassExplanation<'a> {
    /// Format version for machine consumers.
    pub format: &'static str,
    /// Pass metadata.
    pub pass: OptimizationPass,
    /// Runtime reason the pass fired.
    pub reason: &'a str,
    /// Expected effect of the pass on this input.
    pub expected_effect: &'a str,
}

impl OptimizationPassExplanation<'_> {
    /// Stable single-line record for logs and release artifacts.
    #[must_use]
    pub fn stable_record(&self) -> String {
        let mut record = String::new();
        self.stable_record_into(&mut record);
        record
    }

    /// Write the stable single-line record into caller-owned storage.
    pub fn stable_record_into(&self, out: &mut String) {
        use std::fmt::Write as _;

        out.clear();
        let _ = write!(
            out,
            "{}|pass={}|owner={}|phase={}|invariant={}|benchmark={}|reason={}|effect={}",
            self.format,
            self.pass.id,
            self.pass.owner,
            self.pass.phase,
            self.pass.invariant,
            self.pass.benchmark,
            self.reason,
            self.expected_effect
        );
    }
}

#[cfg(test)]
mod pass_order_tests {
    use super::*;

    #[test]
    fn registry_accepts_domain_ordered_release_pipeline() {
        let registry = OptimizationRegistry::with_release_builtins();

        registry
            .validate_pass_order([
                "vyrec.gpu-token-classify",
                "vyrec.parser-recovery",
                "vyrec.semantic-decls",
                "dataflow.graph-layout-hash",
                "dataflow.fixed-point-resident-graph",
                "dataflow.ifds-repeated-sequence",
                "cuda.module-cache",
                "cuda.pinned-readback-pool",
                "cuda.megakernel-memory-budget",
                "cuda.megakernel-barrier-min",
            ])
            .expect("Fix: domain-ordered release pipeline should validate");
    }

    #[test]
    fn registry_rejects_frontend_semantics_before_preprocessing() {
        let registry = OptimizationRegistry::with_release_builtins();
        let err = registry
            .validate_pass_order(["vyrec.semantic-decls", "vyrec.gpu-token-classify"])
            .expect_err("semantic analysis cannot precede preprocessing");

        assert!(err.contains("frontend phase order"), "{err}");
        assert!(err.contains("vyrec.gpu-token-classify"), "{err}");
    }

    #[test]
    fn registry_rejects_dataflow_solve_before_graph_layout() {
        let registry = OptimizationRegistry::with_release_builtins();
        let err = registry
            .validate_pass_order([
                "dataflow.ifds-repeated-sequence",
                "dataflow.graph-layout-hash",
            ])
            .expect_err("IFDS solve cannot precede graph layout");

        assert!(err.contains("dataflow phase order"), "{err}");
        assert!(err.contains("dataflow.graph-layout-hash"), "{err}");
    }

    #[test]
    fn registry_rejects_unknown_and_duplicate_passes() {
        let registry = OptimizationRegistry::with_release_builtins();
        let unknown = registry
            .validate_pass_order(["cuda.module-cache", "cuda.not-registered"])
            .expect_err("unknown pass should not schedule");
        assert!(unknown.contains("unknown optimization pass"), "{unknown}");

        let duplicate = registry
            .validate_pass_order(["cuda.module-cache", "cuda.module-cache"])
            .expect_err("duplicate pass should require an explicit wrapper");
        assert!(duplicate.contains("appears more than once"), "{duplicate}");
    }
}

#[cfg(test)]
mod pass_explanation_tests {
    use super::*;

    #[test]
    fn registry_emits_stable_pass_fire_explanation() {
        let registry = OptimizationRegistry::with_release_builtins();
        let explanation = registry
            .explain_pass_fire(
                "cuda.megakernel-memory-budget",
                "frontier telemetry predicts peak scratch above dense threshold",
                "select bounded plan before launch allocation",
            )
            .expect("Fix: registered pass should explain");

        assert_eq!(explanation.format, "vyre-optimization-explanation-v1");
        assert_eq!(explanation.pass.owner, "vyre-cuda");
        assert!(explanation
            .stable_record()
            .contains("pass=cuda.megakernel-memory-budget"));
        assert!(explanation
            .stable_record()
            .contains("invariant=peak bytes are bounded before launch"));
    }

    #[test]
    fn pass_explanation_reuses_caller_owned_record_storage() {
        let registry = OptimizationRegistry::with_release_builtins();
        let explanation = registry
            .explain_pass_fire(
                "cuda.kernel-failure-diagnostics",
                "capability gate rejected selected launch",
                "emit actionable missing capability list",
            )
            .expect("Fix: registered CUDA diagnostic pass should explain");
        let mut record = String::with_capacity(512);
        let ptr = record.as_ptr();

        explanation.stable_record_into(&mut record);

        assert_eq!(record.as_ptr(), ptr);
        assert!(record.contains("pass=cuda.kernel-failure-diagnostics"));
        assert!(record.contains("effect=emit actionable missing capability list"));

        let mut phase_matches = Vec::with_capacity(8);
        let matches_ptr = phase_matches.as_ptr();
        registry.by_phase_into("cuda-launch", &mut phase_matches);

        assert_eq!(phase_matches.as_ptr(), matches_ptr);
        assert!(phase_matches
            .iter()
            .any(|pass| pass.id == "cuda.kernel-failure-diagnostics"));
    }

    #[test]
    fn registry_rejects_unstable_pass_explanation_fields() {
        let registry = OptimizationRegistry::with_release_builtins();

        let empty = registry
            .explain_pass_fire("cuda.megakernel-memory-budget", "", "bounded allocation")
            .expect_err("empty reason should be rejected");
        assert!(empty.contains("empty reason"), "{empty}");

        let unstable = registry
            .explain_pass_fire(
                "cuda.megakernel-memory-budget",
                "frontier|telemetry",
                "bounded allocation",
            )
            .expect_err("separator should be rejected");
        assert!(unstable.contains("unstable separator"), "{unstable}");
    }
}

/// Extensible registry with duplicate and metadata validation.
#[derive(Clone, Debug, Default)]
pub struct OptimizationRegistry {
    passes: BTreeMap<&'static str, OptimizationPass>,
}

impl OptimizationRegistry {
    /// Registry populated with release-path builtins.
    #[must_use]
    pub fn with_release_builtins() -> Self {
        let mut registry = Self::default();
        for pass in RELEASE_OPTIMIZATION_PASSES {
            let _ = registry.register(*pass);
        }
        registry
    }

    /// Register one pass descriptor.
    pub fn register(&mut self, pass: OptimizationPass) -> Result<(), String> {
        validate_pass(pass)?;
        if self.passes.insert(pass.id, pass).is_some() {
            return Err(format!(
                "optimization registry duplicate pass id `{}`. Fix: choose one stable owner for each optimization.",
                pass.id
            ));
        }
        Ok(())
    }

    /// Get a pass by stable id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&OptimizationPass> {
        self.passes.get(id)
    }

    /// Iterate passes in stable id order.
    pub fn iter(&self) -> impl Iterator<Item = &OptimizationPass> {
        self.passes.values()
    }

    /// Query passes by phase.
    #[must_use]
    pub fn by_phase(&self, phase: &str) -> Vec<&OptimizationPass> {
        let mut passes = Vec::new();
        self.by_phase_into(phase, &mut passes);
        passes
    }

    /// Query passes by phase into caller-owned storage.
    pub fn by_phase_into<'a>(&'a self, phase: &str, out: &mut Vec<&'a OptimizationPass>) {
        out.clear();
        out.extend(self.passes.values().filter(|pass| pass.phase == phase));
    }

    /// Number of registered passes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.passes.len()
    }

    /// True when no passes are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Validate uniqueness and required metadata.
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        for pass in self.passes.values().copied() {
            validate_pass(pass)?;
            if !ids.insert(pass.id) {
                return Err(format!(
                    "optimization registry duplicate pass id `{}`. Fix: pass ids must be globally unique.",
                    pass.id
                ));
            }
        }
        Ok(())
    }

    /// Validate that a proposed optimization sequence respects domain phase order.
    pub fn validate_pass_order<'a, I>(&self, pass_ids: I) -> Result<(), String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut seen = BTreeSet::new();
        let mut latest_by_domain: BTreeMap<&'static str, (&'static str, &'static str, u16)> =
            BTreeMap::new();

        for pass_id in pass_ids {
            if !seen.insert(pass_id) {
                return Err(format!(
                    "optimization pass `{pass_id}` appears more than once. Fix: make repeated passes explicit with a distinct fixed-point wrapper."
                ));
            }

            let pass = self.get(pass_id).ok_or_else(|| {
                format!(
                    "unknown optimization pass `{pass_id}`. Fix: register the pass before scheduling it."
                )
            })?;
            let domain = order_domain(pass.owner);
            let rank = phase_rank(pass.owner, pass.phase).ok_or_else(|| {
                format!(
                    "optimization pass `{}` uses unordered phase `{}`. Fix: add the phase to the registry ordering contract before scheduling it.",
                    pass.id, pass.phase
                )
            })?;

            if let Some((previous_id, previous_phase, previous_rank)) = latest_by_domain.get(domain)
            {
                if rank < *previous_rank {
                    return Err(format!(
                        "optimization pass `{}` phase `{}` cannot run after `{}` phase `{}`. Fix: preserve {} phase order.",
                        pass.id, pass.phase, previous_id, previous_phase, domain
                    ));
                }
            }

            latest_by_domain.insert(domain, (pass.id, pass.phase, rank));
        }

        Ok(())
    }

    /// Build a stable explanation for a fired pass.
    pub fn explain_pass_fire<'a>(
        &self,
        pass_id: &str,
        reason: &'a str,
        expected_effect: &'a str,
    ) -> Result<OptimizationPassExplanation<'a>, String> {
        validate_stable_field("reason", reason)?;
        validate_stable_field("expected_effect", expected_effect)?;
        let pass = self.get(pass_id).ok_or_else(|| {
            format!(
                "unknown optimization pass `{pass_id}`. Fix: register the pass before explaining it."
            )
        })?;

        Ok(OptimizationPassExplanation {
            format: "vyre-optimization-explanation-v1",
            pass: *pass,
            reason,
            expected_effect,
        })
    }
}

fn validate_pass(pass: OptimizationPass) -> Result<(), String> {
    for (field, value) in [
        ("id", pass.id),
        ("owner", pass.owner),
        ("phase", pass.phase),
        ("invariant", pass.invariant),
        ("benchmark", pass.benchmark),
    ] {
        if value.trim().is_empty() {
            return Err(format!(
                "optimization pass `{}` has empty {field}. Fix: every pass needs owner, phase, invariant, and benchmark metadata.",
                pass.id
            ));
        }
    }
    if phase_rank(pass.owner, pass.phase).is_none() {
        return Err(format!(
            "optimization pass `{}` uses unordered phase `{}` for owner `{}`. Fix: add the phase to the registry ordering contract before release.",
            pass.id, pass.phase, pass.owner
        ));
    }
    Ok(())
}

fn validate_stable_field(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "optimization explanation has empty {field}. Fix: explain why the pass fired and what effect it should have."
        ));
    }
    if value
        .bytes()
        .any(|byte| byte == b'|' || byte == b'\n' || byte == b'\r' || byte == b'\t')
    {
        return Err(format!(
            "optimization explanation {field} contains an unstable separator. Fix: use plain single-line text without pipe or tab separators."
        ));
    }
    Ok(())
}

fn order_domain(owner: &'static str) -> &'static str {
    match owner {
        "vyrec" => "frontend",
        "dataflow" => "dataflow",
        "vyre-cuda" | "vyre-driver" | "vyre-primitives" | "vyre-self" => "runtime",
        _ => "release",
    }
}

fn phase_rank(owner: &'static str, phase: &'static str) -> Option<u16> {
    match order_domain(owner) {
        "frontend" => match phase {
            "preprocessing" => Some(10),
            "diagnostics" => Some(20),
            "parsing" => Some(30),
            "semantic-analysis" => Some(40),
            "frontend-tests" | "frontend-fuzz" => Some(80),
            "frontend-release" => Some(90),
            _ => None,
        },
        "dataflow" => match phase {
            "graph-layout" => Some(10),
            "dataflow-safety" => Some(20),
            "dataflow-residency" | "ifds-residency" => Some(30),
            "dataflow-planning" => Some(40),
            "dataflow-dispatch" => Some(50),
            "dataflow-batch" | "dataflow-output" | "ifds-solve" | "reaching" | "live"
            | "points-to" | "slice" | "bitset" => Some(60),
            "dataflow-tests" => Some(80),
            "dataflow-benchmark" => Some(90),
            _ => None,
        },
        "runtime" => match phase {
            "architecture" => Some(0),
            "api" => Some(5),
            "aot" | "binding" | "cuda-jit" | "optimizer" => Some(10),
            "cuda-memory"
            | "cuda-resident-io"
            | "cuda-resident-sequence"
            | "memory"
            | "bitset"
            | "math"
            | "reduce"
            | "hash"
            | "encoding"
            | "tensor"
            | "text" => Some(20),
            "lowering" | "dispatch-routing" | "pipeline" => Some(30),
            "graph" => Some(35),
            "cuda-launch" => Some(40),
            "diagnostics" => Some(45),
            "cuda-graph" | "megakernel-cache" | "megakernel-memory" => Some(50),
            "megakernel-scheduler" | "graph-traversal" => Some(60),
            "benchmark" => Some(90),
            _ => None,
        },
        "release" => match phase {
            "release" | "release-gate" => Some(100),
            _ => Some(100),
        },
        _ => None,
    }
}

pub use super::optimization_release_passes::RELEASE_OPTIMIZATION_PASSES;

#[cfg(test)]
mod tests {
    use super::{OptimizationPass, OptimizationRegistry, RELEASE_OPTIMIZATION_PASSES};

    #[test]
    fn release_registry_has_at_least_one_hundred_discoverable_passes() {
        let registry = OptimizationRegistry::with_release_builtins();

        assert!(
            registry.len() >= 100,
            "Fix: release optimization registry must enumerate at least 100 concrete passes; got {}.",
            registry.len()
        );
        registry
            .validate()
            .expect("Fix: release optimization registry metadata must be complete and unique.");
    }

    #[test]
    fn release_registry_covers_cuda_dataflow_frontend_and_release_gates() {
        let registry = OptimizationRegistry::with_release_builtins();

        for phase in [
            "cuda-resident-io",
            "megakernel-scheduler",
            "dataflow-residency",
            "preprocessing",
            "semantic-analysis",
            "release-gate",
        ] {
            assert_ne!(
                registry.by_phase(phase).len(),
                0,
                "Fix: optimization registry must expose phase `{phase}`."
            );
        }
    }

    #[test]
    fn registry_rejects_duplicate_or_empty_metadata() {
        let mut registry = OptimizationRegistry::default();
        let pass = RELEASE_OPTIMIZATION_PASSES[0];
        registry.register(pass).expect("Fix: first pass registers");
        assert!(
            matches!(registry.register(pass), Err(_)),
            "Fix: duplicate optimization pass ids must be rejected."
        );
        assert!(
            registry
                .register(OptimizationPass {
                    id: "bad.empty-owner",
                    owner: "",
                    phase: "phase",
                    invariant: "invariant",
                    benchmark: "bench",
                })
                .is_err(),
            "Fix: empty optimization metadata must be rejected."
        );
    }

    #[test]
    fn registry_rejects_unordered_release_phase_metadata() {
        let mut registry = OptimizationRegistry::default();
        let err = registry
            .register(OptimizationPass {
                id: "bad.unordered-phase",
                owner: "vyre-cuda",
                phase: "mystery-phase",
                invariant: "phase must be ordered",
                benchmark: "phase_order_contract",
            })
            .expect_err("unordered release phases must be rejected");

        assert!(err.contains("unordered phase"), "{err}");
        OptimizationRegistry::with_release_builtins()
            .validate()
            .expect("Fix: all built-in release optimization phases must be ordered");
    }
}
