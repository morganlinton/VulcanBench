use super::model::{
    InnovationCoverage, VxRow, BASELINE_MARKERS, MAX_FINDINGS, NEGATIVE_CASE_MARKERS,
};
use crate::innovation_falsification::missing_frontier_falsification_fields;
use crate::research_key::backtick_research_keys;

pub(super) fn collect_innovation_coverage(rows: &[VxRow]) -> Vec<InnovationCoverage> {
    let mut coverage = Vec::new();
    for row in rows {
        if !row.work.starts_with("Innovation candidate:") {
            continue;
        }
        let research_keys = backtick_research_keys(&row.research_basis);
        let has_named_external_source = !research_keys.is_empty();
        let owner_lane = owner_lane(&row.axis);
        let combined =
            format!("{} {} {}", row.work, row.proof_gate, row.local_evidence).to_ascii_lowercase();
        let baseline_type = baseline_type(&combined);
        let workload_family = workload_family(&row.axis);
        let has_local_path_evidence = has_local_path_evidence(&row.local_evidence);
        let negative_case_family = negative_case_family(&combined);
        let gpu_claim_policy = gpu_claim_policy(&combined, workload_family);
        let has_gpu_partition_rationale = has_gpu_partition_rationale(&combined);
        let has_transfer_accounting = has_transfer_accounting(&combined);
        let has_baseline_field = baseline_type != "missing";
        let mut missing = Vec::new();
        if !has_named_external_source {
            missing.push("named-external-source".to_string());
        }
        if owner_lane == "missing" {
            missing.push("owner-lane".to_string());
        }
        if baseline_type == "missing" {
            missing.push("baseline-type".to_string());
        }
        if workload_family == "unknown" {
            missing.push("workload-family".to_string());
        }
        if !has_local_path_evidence {
            missing.push("local-path-evidence".to_string());
        }
        if negative_case_family == "missing" {
            missing.push("negative-case-family".to_string());
        }
        if gpu_claim_policy == "requires-partition-transfer-baseline" {
            if !has_gpu_partition_rationale {
                missing.push("gpu-partition-rationale".to_string());
            }
            if !has_transfer_accounting {
                missing.push("gpu-transfer-accounting".to_string());
            }
            if !has_baseline_field {
                missing.push("gpu-baseline-field".to_string());
            }
        }
        missing.extend(
            missing_frontier_falsification_fields(&row.id, &combined)
                .into_iter()
                .map(|field| format!("falsification-{field}")),
        );
        coverage.push(InnovationCoverage {
            vx_id: row.id.clone(),
            axis: row.axis.clone(),
            research_keys,
            has_named_external_source,
            owner_lane,
            baseline_type: baseline_type.to_string(),
            workload_family: workload_family.to_string(),
            has_local_path_evidence,
            negative_case_family: negative_case_family.to_string(),
            gpu_claim_policy: gpu_claim_policy.to_string(),
            has_gpu_partition_rationale,
            has_transfer_accounting,
            has_baseline_field,
            grounding_severity: grounding_severity(&missing).to_string(),
            missing,
        });
        if coverage.len() >= MAX_FINDINGS {
            break;
        }
    }
    coverage
}

fn gpu_claim_policy(text: &str, workload_family: &str) -> &'static str {
    let gpu_claim = text.contains("gpu")
        || text.contains("cuda")
        || text.contains("wgpu")
        || text.contains("metal")
        || text.contains("accelerator");
    let sensitive_family = matches!(workload_family, "scan" | "parser" | "flow-security" | "graph");
    if gpu_claim && sensitive_family {
        "requires-partition-transfer-baseline"
    } else {
        "not-gpu-claim"
    }
}

fn has_gpu_partition_rationale(text: &str) -> bool {
    text.contains("partition")
        || text.contains("cpu/gpu")
        || text.contains("routing")
        || text.contains("classifier")
        || text.contains("selected")
        || text.contains("rejected")
        || text.contains("fallback")
}

fn has_transfer_accounting(text: &str) -> bool {
    text.contains("transfer")
        || text.contains("bytes")
        || text.contains("host-copy")
        || text.contains("host copy")
        || text.contains("upload")
        || text.contains("readback")
}

fn owner_lane(axis: &str) -> String {
    let axis = axis.trim();
    if axis.is_empty() {
        "missing".to_string()
    } else if axis.contains("runtime") {
        "runtime".to_string()
    } else if axis.contains("driver") {
        "driver".to_string()
    } else if axis.contains("scan") {
        "scan".to_string()
    } else if axis.contains("parser") {
        "parser".to_string()
    } else if axis.contains("flow") || axis.contains("dataflow") || axis.contains("security") {
        "flow-security".to_string()
    } else if axis.contains("graph") {
        "graph".to_string()
    } else if axis.contains("sparse") || axis.contains("nn") {
        "math-ai".to_string()
    } else if axis.contains("compiler") || axis.contains("optimizer") || axis.contains("lower") {
        "compiler".to_string()
    } else if axis.contains("bench") {
        "benchmark".to_string()
    } else if axis.contains("product") {
        "product".to_string()
    } else if axis.contains("evidence") || axis.contains("testing") || axis.contains("coordination") {
        "evidence".to_string()
    } else {
        axis.to_string()
    }
}

fn grounding_severity(missing: &[String]) -> &'static str {
    if missing.is_empty() {
        "clean"
    } else if missing.len() <= 2 {
        "warning"
    } else {
        "blocker"
    }
}

fn baseline_type(text: &str) -> &'static str {
    if text.contains("differential") {
        "differential"
    } else if text.contains("baseline") {
        "baseline"
    } else if text.contains("bench") {
        "benchmark"
    } else if text.contains("compare") || text.contains("against") {
        "comparison"
    } else if text.contains("parity") {
        "parity"
    } else if BASELINE_MARKERS.iter().any(|marker| text.contains(*marker)) {
        "baseline-marker"
    } else {
        "missing"
    }
}

fn workload_family(axis: &str) -> &'static str {
    if axis.contains("scan") {
        "scan"
    } else if axis.contains("parser") {
        "parser"
    } else if axis.contains("flow") || axis.contains("dataflow") || axis.contains("security") {
        "flow-security"
    } else if axis.contains("graph") {
        "graph"
    } else if axis.contains("runtime") {
        "runtime"
    } else if axis.contains("driver") {
        "driver"
    } else if axis.contains("sparse") || axis.contains("nn") {
        "math-ai"
    } else if axis.contains("compiler") || axis.contains("optimizer") || axis.contains("lower") {
        "compiler"
    } else if axis.contains("bench") {
        "benchmark"
    } else if axis.contains("product") {
        "product"
    } else if axis.contains("evidence") || axis.contains("testing") || axis.contains("coordination") {
        "evidence"
    } else {
        "unknown"
    }
}

fn has_local_path_evidence(text: &str) -> bool {
    markdown_path_tokens(text).any(|token| {
        token == "Cargo.toml"
            || token.contains('/')
            || token.ends_with(".rs")
            || token.ends_with(".md")
            || token.ends_with(".toml")
            || token.ends_with(".json")
    })
}

fn markdown_path_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split('`')
        .enumerate()
        .filter_map(|(index, token)| (index % 2 == 1).then_some(token))
}

fn negative_case_family(text: &str) -> &'static str {
    if text.contains("adversarial") {
        "adversarial"
    } else if text.contains("unsupported") || text.contains("reject") || text.contains("fail") {
        "negative-diagnostic"
    } else if text.contains("drift") || text.contains("tolerance") {
        "numeric-drift"
    } else if text.contains("parity") || text.contains("identical") || text.contains("exact") {
        "negative-twin-or-parity"
    } else if text.contains("digest") || text.contains("witness") {
        "correctness-digest-or-witness"
    } else if NEGATIVE_CASE_MARKERS.iter().any(|marker| text.contains(*marker)) {
        "negative-marker"
    } else {
        "missing"
    }
}
