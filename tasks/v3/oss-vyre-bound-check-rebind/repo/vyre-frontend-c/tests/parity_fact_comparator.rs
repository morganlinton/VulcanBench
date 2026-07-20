//! Tests for deterministic clang/vyrec parity fact comparison.

use vyre_frontend_c::api::{
    compare_parity_facts, ParityComparableFact, ParityFactCategory, ParityFindingKind,
    ParitySourceProvenance,
};

#[test]
fn comparator_classifies_fact_mismatches_without_raw_diffs() {
    let clang = vec![
        fact("a:match", "digest-a", "/tmp/a.c:1:1"),
        fact("b:span", "digest-b", "/tmp/a.c:2:1"),
        fact("c:semantic", "digest-c", "/tmp/a.c:3:1"),
        fact("d:missing", "digest-d", "/tmp/a.c:4:1"),
    ];
    let vyrec = vec![
        fact("a:match", "digest-a", "/tmp/a.c:1:1"),
        fact("b:span", "digest-b", "/tmp/a.c:20:1"),
        fact("c:semantic", "different", "/tmp/a.c:3:1"),
        fact("e:extra", "digest-e", "/tmp/a.c:5:1"),
    ];

    let report = compare_parity_facts(
        "linux-lib-math-v6.8",
        "90d1f30371ae3337beb01666b226320728d35c70",
        "clang",
        "vyrec",
        "NVIDIA GeForce RTX 5090",
        "resident-graph",
        clang,
        vyrec,
    );

    let observed = report
        .findings()
        .iter()
        .map(|finding| (finding.fact_id.as_str(), finding.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("a:match", ParityFindingKind::Match),
            ("b:span", ParityFindingKind::SpanMismatch),
            ("c:semantic", ParityFindingKind::SemanticMismatch),
            ("d:missing", ParityFindingKind::VyrecMissing),
            ("e:extra", ParityFindingKind::VyrecExtra),
        ]
    );
    assert_eq!(report.blocking_findings().len(), 4);
}

fn fact(id: &str, digest: &str, location: &str) -> ParityComparableFact {
    ParityComparableFact::new(
        ParityFactCategory::SemanticAnalysis,
        id,
        digest,
        ParitySourceProvenance::from_clang_locations(location, None, [] as [&str; 0]),
    )
}
