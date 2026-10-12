use super::*;
/// Compares clang and vyrec comparable facts into a deterministic release report.
#[must_use]
pub fn compare_parity_facts(
    target_id: impl Into<String>,
    source_commit: impl Into<String>,
    clang_version: impl Into<String>,
    vyrec_version: impl Into<String>,
    gpu: impl Into<String>,
    mode: impl Into<String>,
    clang_facts: impl IntoIterator<Item = ParityComparableFact>,
    vyrec_facts: impl IntoIterator<Item = ParityComparableFact>,
) -> ParityReleaseReport {
    let mut report = ParityReleaseReport::new(
        target_id,
        source_commit,
        clang_version,
        vyrec_version,
        gpu,
        mode,
    );
    let clang = keyed_facts(clang_facts);
    let vyrec = keyed_facts(vyrec_facts);
    for (fact_id, clang_fact) in &clang {
        match vyrec.get(fact_id) {
            Some(vyrec_fact)
                if clang_fact.semantic_digest == vyrec_fact.semantic_digest
                    && clang_fact.provenance == vyrec_fact.provenance =>
            {
                report.push_match(
                    clang_fact.category,
                    fact_id.clone(),
                    "semantic digest and source provenance match",
                );
            }
            Some(vyrec_fact) if clang_fact.semantic_digest == vyrec_fact.semantic_digest => {
                report.push_finding(ParityFinding::new(
                    clang_fact.category,
                    ParityFindingKind::SpanMismatch,
                    fact_id.clone(),
                    "semantic digest matches but normalized source provenance differs",
                ));
            }
            Some(_) => {
                report.push_finding(ParityFinding::new(
                    clang_fact.category,
                    ParityFindingKind::SemanticMismatch,
                    fact_id.clone(),
                    "semantic digest differs",
                ));
            }
            None => {
                report.push_finding(ParityFinding::new(
                    clang_fact.category,
                    ParityFindingKind::VyrecMissing,
                    fact_id.clone(),
                    "required clang fact is missing from vyrec facts",
                ));
            }
        }
    }
    for (fact_id, vyrec_fact) in &vyrec {
        if !clang.contains_key(fact_id) {
            report.push_finding(ParityFinding::new(
                vyrec_fact.category,
                ParityFindingKind::VyrecExtra,
                fact_id.clone(),
                "vyrec produced a fact not present in clang oracle facts",
            ));
        }
    }
    report
}

fn keyed_facts(
    facts: impl IntoIterator<Item = ParityComparableFact>,
) -> BTreeMap<String, ParityComparableFact> {
    facts
        .into_iter()
        .map(|fact| (fact.fact_id.clone(), fact))
        .collect()
}
