use super::*;

#[test]
fn readme_benchmark_section_leads_with_cuda_macro_release_evidence() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Fix: vyre-bench must live under the workspace root.");
    let readme = std::fs::read_to_string(workspace.join("README.md"))
        .expect("Fix: README.md must remain readable.");
    let section = readme
        .split("## Benchmarks\n")
        .nth(1)
        .expect("Fix: README.md must contain a Benchmarks section.")
        .split("Auto-registration is handled by link-time")
        .next()
        .expect("Fix: README.md benchmark section must precede registration docs.");

    assert!(
        section.contains("release/evidence/benchmarks/cuda-release-suite.json"),
        "Fix: README benchmark claims must point at CUDA release-suite evidence."
    );
    assert!(
        section.contains("16 macro workload families")
            && section.contains("explicit CPU-SOTA release contracts"),
        "Fix: README benchmark section must lead with macro release workloads and CPU-SOTA release contracts."
    );
    for required_case in [
        "release.condition_eval.1m",
        "release.string_bitmap_scatter.1m",
        "release.offset_count_aggregation.1m",
        "conditions.yara_like.eval.1m",
        "release.entropy_window.1m",
        "release.quantified_condition_loops.1m",
        "release.alias_reaching_def.1m",
        "release.ifds_witness.1m",
        "release.c_ast_traversal.1m",
        "release.megakernel_queue.1m",
        "release.egraph_saturation.1m",
        "sparse.compaction.count.1m",
        "callgraph.reachability.step.262k",
    ] {
        assert!(
            section.contains(required_case),
            "Fix: README benchmark section must include release case `{required_case}`."
        );
    }
    assert!(
        !section.contains("| primitive.") && !section.contains(">1048576"),
        "Fix: README benchmark section must not resurrect the stale primitive-only crossover table."
    );
}
