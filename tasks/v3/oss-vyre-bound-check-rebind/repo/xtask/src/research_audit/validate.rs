use super::model::{
    COMMAND_MATRIX_PATH, MIN_PLAN_ROWS, PLAN_PATH, RAW_COUNTER_FAMILIES, SCHEMA_VERSION,
    SOURCE_DIGEST_PREFIX,
};
use super::{
    research_audit_required_artifact_fields, RESEARCH_AUDIT_REQUIRED_ARRAY_FIELDS,
    RESEARCH_AUDIT_REQUIRED_POSITIVE_COUNT_FIELDS,
};

pub(crate) fn validate_research_audit_artifact_bytes(
    bytes: &[u8],
    expected_generator_command: &str,
) -> Vec<String> {
    let mut blockers = Vec::new();
    let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => value,
        Err(error) => {
            return vec![format!("research audit artifact is not valid JSON: {error}")];
        }
    };
    if value.get("schema_version").and_then(|raw| raw.as_u64()) != Some(SCHEMA_VERSION.into()) {
        blockers.push(format!(
            "research audit artifact must use schema_version={SCHEMA_VERSION}"
        ));
    }
    if value
        .get("generator_command")
        .and_then(|raw| raw.as_str())
        != Some(expected_generator_command)
    {
        blockers.push(
            "research audit artifact generator_command must match release evidence command"
                .to_string(),
        );
    }
    for field in research_audit_required_artifact_fields() {
        if value.get(field).is_none() {
            blockers.push(format!("research audit artifact `{field}` is missing"));
        }
    }
    if value.get("plan_path").and_then(|raw| raw.as_str()) != Some(PLAN_PATH) {
        blockers.push(format!(
            "research audit artifact plan_path must be `{PLAN_PATH}`"
        ));
    }
    if value
        .get("command_matrix_path")
        .and_then(|raw| raw.as_str())
        != Some(COMMAND_MATRIX_PATH)
    {
        blockers.push(format!(
            "research audit artifact command_matrix_path must be `{COMMAND_MATRIX_PATH}`"
        ));
    }
    let plan_row_count = value
        .get("plan_row_count")
        .and_then(|raw| raw.as_u64())
        .unwrap_or_default();
    let minimum_plan_row_count = value
        .get("minimum_plan_row_count")
        .and_then(|raw| raw.as_u64())
        .unwrap_or_default();
    if minimum_plan_row_count != MIN_PLAN_ROWS as u64 {
        blockers.push(format!(
            "research audit artifact minimum_plan_row_count must equal shared VX floor {MIN_PLAN_ROWS}"
        ));
    }
    let effective_minimum_plan_row_count = minimum_plan_row_count.max(MIN_PLAN_ROWS as u64);
    if plan_row_count < effective_minimum_plan_row_count {
        blockers.push(format!(
            "research audit artifact plan_row_count must be at least {effective_minimum_plan_row_count}"
        ));
    }
    for field in RESEARCH_AUDIT_REQUIRED_POSITIVE_COUNT_FIELDS {
        if value
            .get(*field)
            .and_then(|raw| raw.as_u64())
            .unwrap_or_default()
            == 0
        {
            blockers.push(format!(
                "research audit artifact `{field}` must be a positive count"
            ));
        }
    }
    for field in RESEARCH_AUDIT_REQUIRED_ARRAY_FIELDS {
        if !value.get(*field).is_some_and(|raw| raw.is_array()) {
            blockers.push(format!("research audit artifact `{field}` must be an array"));
        }
    }
    match value.get("raw_counter_families").and_then(|raw| raw.as_array()) {
        Some(families) => {
            for required in RAW_COUNTER_FAMILIES {
                let present = families
                    .iter()
                    .filter_map(|raw| raw.as_str())
                    .any(|family| family == *required);
                if !present {
                    blockers.push(format!(
                        "research audit artifact raw_counter_families is missing `{required}`"
                    ));
                }
            }
        }
        None => blockers
            .push("research audit artifact `raw_counter_families` must be an array".to_string()),
    }
    match value.get("blockers").and_then(|raw| raw.as_array()) {
        Some(blockers_value) if blockers_value.is_empty() => {}
        Some(blockers_value) => blockers.push(format!(
            "research audit artifact contains {} blocker(s)",
            blockers_value.len()
        )),
        None => blockers.push("research audit artifact `blockers` must be an array".to_string()),
    }
    if let Some(boundary_findings) = value
        .get("repo_boundary_findings")
        .and_then(|raw| raw.as_array())
    {
        if !boundary_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} repository-boundary finding(s)",
                boundary_findings.len()
            ));
        }
    }
    if let Some(protocol_findings) = value
        .get("megakernel_protocol_boundary_findings")
        .and_then(|raw| raw.as_array())
    {
        if !protocol_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} megakernel-protocol-boundary finding(s)",
                protocol_findings.len()
            ));
        }
    }
    if let Some(baseline_gaps) = value.get("baseline_gaps").and_then(|raw| raw.as_array()) {
        if !baseline_gaps.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} baseline-gap finding(s)",
                baseline_gaps.len()
            ));
        }
    }
    if let Some(innovation_coverage) = value
        .get("innovation_coverage")
        .and_then(|raw| raw.as_array())
    {
        for (index, coverage) in innovation_coverage.iter().enumerate() {
            let severity = coverage
                .get("grounding_severity")
                .and_then(|raw| raw.as_str());
            if !matches!(severity, Some("clean" | "warning" | "blocker")) {
                blockers.push(format!(
                    "research audit artifact innovation_coverage[{index}].grounding_severity must be clean, warning, or blocker"
                ));
            }
            if coverage
                .get("has_named_external_source")
                .and_then(|raw| raw.as_bool())
                != Some(true)
            {
                blockers.push(format!(
                    "research audit artifact innovation_coverage[{index}] is missing named external source evidence"
                ));
            }
            if !coverage
                .get("owner_lane")
                .and_then(|raw| raw.as_str())
                .is_some_and(|lane| !lane.is_empty() && lane != "missing")
            {
                blockers.push(format!(
                    "research audit artifact innovation_coverage[{index}] is missing owner lane"
                ));
            }
            let gpu_claim_policy = coverage
                .get("gpu_claim_policy")
                .and_then(|raw| raw.as_str());
            if !matches!(
                gpu_claim_policy,
                Some("requires-partition-transfer-baseline" | "not-gpu-claim")
            ) {
                blockers.push(format!(
                    "research audit artifact innovation_coverage[{index}].gpu_claim_policy is invalid"
                ));
            }
            if gpu_claim_policy == Some("requires-partition-transfer-baseline") {
                if coverage
                    .get("has_gpu_partition_rationale")
                    .and_then(|raw| raw.as_bool())
                    != Some(true)
                {
                    blockers.push(format!(
                        "research audit artifact innovation_coverage[{index}] is missing GPU partition rationale"
                    ));
                }
                if coverage
                    .get("has_transfer_accounting")
                    .and_then(|raw| raw.as_bool())
                    != Some(true)
                {
                    blockers.push(format!(
                        "research audit artifact innovation_coverage[{index}] is missing transfer-byte accounting"
                    ));
                }
                if coverage
                    .get("has_baseline_field")
                    .and_then(|raw| raw.as_bool())
                    != Some(true)
                {
                    blockers.push(format!(
                        "research audit artifact innovation_coverage[{index}] is missing GPU baseline field"
                    ));
                }
            }
            match coverage.get("missing").and_then(|raw| raw.as_array()) {
                Some(missing) if missing.is_empty() => {}
                Some(missing) => blockers.push(format!(
                    "research audit artifact innovation_coverage[{index}] is missing {} research-grounding dimension(s)",
                    missing.len()
                )),
                None => blockers.push(format!(
                    "research audit artifact innovation_coverage[{index}].missing must be an array"
                )),
            }
        }
    }
    if let Some(script_findings) = value
        .get("script_policy_findings")
        .and_then(|raw| raw.as_array())
    {
        if !script_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} script-policy finding(s)",
                script_findings.len()
            ));
        }
    }
    if let Some(loader_findings) = value
        .get("rust_toml_loader_findings")
        .and_then(|raw| raw.as_array())
    {
        if !loader_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} Rust TOML loader finding(s)",
                loader_findings.len()
            ));
        }
    }
    if let Some(source_ledger_findings) = value
        .get("source_ledger_findings")
        .and_then(|raw| raw.as_array())
    {
        if !source_ledger_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} research-source-ledger finding(s)",
                source_ledger_findings.len()
            ));
        }
    }
    if let Some(competitor_issue_findings) = value
        .get("competitor_issue_findings")
        .and_then(|raw| raw.as_array())
    {
        if !competitor_issue_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} competitor-issue-ledger finding(s)",
                competitor_issue_findings.len()
            ));
        }
    }
    if let Some(research_plan_coverage_findings) = value
        .get("research_plan_coverage_findings")
        .and_then(|raw| raw.as_array())
    {
        if !research_plan_coverage_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} research-plan coverage finding(s)",
                research_plan_coverage_findings.len()
            ));
        }
    }
    if let Some(archive_replay_findings) = value
        .get("archive_replay_findings")
        .and_then(|raw| raw.as_array())
    {
        if !archive_replay_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} archive-replay finding(s)",
                archive_replay_findings.len()
            ));
        }
    }
    if let Some(rules_as_data_findings) = value
        .get("rules_as_data_findings")
        .and_then(|raw| raw.as_array())
    {
        if !rules_as_data_findings.is_empty() {
            blockers.push(format!(
                "research audit artifact contains {} rules-as-data finding(s)",
                rules_as_data_findings.len()
            ));
        }
    }
    if !value
        .get("source_digest")
        .and_then(|raw| raw.as_str())
        .is_some_and(|digest| digest.starts_with(SOURCE_DIGEST_PREFIX))
    {
        blockers.push("research audit artifact source_digest is missing or unstable".to_string());
    }
    if let Some(linkage) = value
        .get("high_risk_vx_linkage")
        .and_then(|raw| raw.as_array())
    {
        for (index, finding) in linkage.iter().enumerate() {
            if finding
                .get("covered_by_vx_row")
                .and_then(|raw| raw.as_bool())
                != Some(true)
            {
                blockers.push(format!(
                    "research audit artifact high_risk_vx_linkage[{index}] is not covered by a VX row"
                ));
            }
        }
    }
    blockers
}

#[cfg(test)]
mod tests {
    use super::super::{
        RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND, RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX,
    };
    use super::*;

    fn complete_raw_counter_families_json() -> String {
        serde_json::to_string(RAW_COUNTER_FAMILIES)
            .expect("Fix: research-audit raw counter families must serialize.")
    }

    fn artifact_fixture(raw_counter_families: &str, rust_toml_loader_findings: &str) -> String {
        artifact_fixture_with_source_ledger(
            raw_counter_families,
            rust_toml_loader_findings,
            "[]",
            "[]",
        )
    }

    fn artifact_fixture_with_innovation(
        raw_counter_families: &str,
        rust_toml_loader_findings: &str,
        innovation_coverage: &str,
    ) -> String {
        artifact_fixture_with_source_ledger(
            raw_counter_families,
            rust_toml_loader_findings,
            innovation_coverage,
            "[]",
        )
    }

    fn artifact_fixture_with_source_ledger(
        raw_counter_families: &str,
        rust_toml_loader_findings: &str,
        innovation_coverage: &str,
        source_ledger_findings: &str,
    ) -> String {
        artifact_fixture_with_competitor_issues(
            raw_counter_families,
            rust_toml_loader_findings,
            innovation_coverage,
            source_ledger_findings,
            "[]",
        )
    }

    fn artifact_fixture_with_competitor_issues(
        raw_counter_families: &str,
        rust_toml_loader_findings: &str,
        innovation_coverage: &str,
        source_ledger_findings: &str,
        competitor_issue_findings: &str,
    ) -> String {
        format!(
            r#"{{
  "schema_version": 6,
  "generator_command": "{RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND}",
  "plan_path": "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md",
  "command_matrix_path": "docs/optimization/XTASK_COMMAND_MATRIX.md",
  "plan_row_count": 480,
  "minimum_plan_row_count": 480,
  "axis_count": 16,
  "defined_research_key_count": 24,
  "used_research_key_count": 24,
  "loc_hotspots": [],
  "claim_drift": [],
  "baseline_gaps": [],
  "innovation_coverage": {innovation_coverage},
  "high_risk_vx_linkage": [],
  "stale_doc_markers": [],
  "repo_boundary_findings": [],
  "megakernel_protocol_boundary_findings": [],
  "script_policy_findings": [],
  "rust_toml_loader_findings": {rust_toml_loader_findings},
  "source_ledger_findings": {source_ledger_findings},
  "competitor_issue_findings": {competitor_issue_findings},
  "research_plan_coverage_findings": [],
  "archive_replay_findings": [],
  "rules_as_data_findings": [],
  "raw_counter_families": {raw_counter_families},
  "blockers": [],
  "source_digest": "{RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX}abc"
}}"#
        )
    }

    #[test]
    fn research_audit_artifact_rejects_missing_innovation_coverage_dimensions() {
        let innovation_coverage = r#"[
    {
      "vx_id": "VX-301",
      "axis": "scan_automata",
      "research_keys": [],
      "has_named_external_source": false,
      "owner_lane": "scan",
      "baseline_type": "missing",
      "workload_family": "scan",
      "has_local_path_evidence": false,
      "negative_case_family": "missing",
      "gpu_claim_policy": "requires-partition-transfer-baseline",
      "has_gpu_partition_rationale": false,
      "has_transfer_accounting": false,
      "has_baseline_field": false,
      "grounding_severity": "warning",
      "missing": ["source-key", "baseline-type"]
    }
  ]"#;
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact =
            artifact_fixture_with_innovation(&raw_counter_families, "[]", innovation_coverage);

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("innovation_coverage[0]")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("named external source")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("GPU partition rationale")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("transfer-byte accounting")));
    }

    #[test]
    fn research_audit_artifact_rejects_missing_raw_counter_family() {
        let artifact = artifact_fixture(r#"["loc_hotspots"]"#, "[]");

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("raw_counter_families")));
    }

    #[test]
    fn research_audit_artifact_rejects_rust_toml_loader_findings() {
        let rust_toml_loader_findings = r#"[
    {
      "path": "xtask/src/release_train.rs",
      "line": 41,
      "text": "toml::from_str::<ReleaseTrainData>(RELEASE_TRAIN_TOML)",
      "policy": "use xtask/src/toml_config.rs for embedded TOML parsing"
    }
  ]"#;
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = artifact_fixture(&raw_counter_families, rust_toml_loader_findings);

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("Rust TOML loader finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_source_ledger_findings() {
        let source_ledger_findings = r#"[
    {
      "path": "docs/optimization/RESEARCH_SOURCE_LEDGER.toml",
      "key": "VECTOR_MATON",
      "text": "research key `VECTOR_MATON` is missing from docs/optimization/RESEARCH_SOURCE_LEDGER.toml",
      "policy": "research-source-ledger-required-key"
    }
  ]"#;
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact =
            artifact_fixture_with_source_ledger(&raw_counter_families, "[]", "[]", source_ledger_findings);

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("research-source-ledger finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_competitor_issue_findings() {
        let competitor_issue_findings = r#"[
    {
      "path": "docs/optimization/COMPETITOR_ISSUE_LEDGER.toml",
      "key": "HYPERSCAN-ISSUE-68",
      "text": "competitor issue links unknown VX row `VX-999`",
      "policy": "competitor-issue-vx-row-exists"
    }
  ]"#;
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = artifact_fixture_with_competitor_issues(
            &raw_counter_families,
            "[]",
            "[]",
            "[]",
            competitor_issue_findings,
        );

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("competitor-issue-ledger finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_megakernel_protocol_boundary_findings() {
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = format!(
            r#"{{
  "schema_version": 6,
  "generator_command": "{RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND}",
  "plan_path": "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md",
  "command_matrix_path": "docs/optimization/XTASK_COMMAND_MATRIX.md",
  "plan_row_count": 480,
  "minimum_plan_row_count": 480,
  "axis_count": 16,
  "defined_research_key_count": 24,
  "used_research_key_count": 24,
  "loc_hotspots": [],
  "claim_drift": [],
  "baseline_gaps": [],
  "innovation_coverage": [],
  "high_risk_vx_linkage": [],
  "stale_doc_markers": [],
  "repo_boundary_findings": [],
  "megakernel_protocol_boundary_findings": [
    {{
      "path": "vyre-driver-wgpu/src/megakernel.rs",
      "key": "megakernel-protocol-boundary",
      "text": "line 16: use vyre_runtime::megakernel::protocol;",
      "policy": "driver megakernel code must use runtime Megakernel API wrappers instead of protocol internals"
    }}
  ],
  "script_policy_findings": [],
  "rust_toml_loader_findings": [],
  "source_ledger_findings": [],
  "competitor_issue_findings": [],
  "research_plan_coverage_findings": [],
  "archive_replay_findings": [],
  "rules_as_data_findings": [],
  "raw_counter_families": {raw_counter_families},
  "blockers": [],
  "source_digest": "{RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX}abc"
}}"#
        );

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("megakernel-protocol-boundary finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_archive_replay_findings() {
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = format!(
            r#"{{
  "schema_version": 6,
  "generator_command": "{RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND}",
  "plan_path": "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md",
  "command_matrix_path": "docs/optimization/XTASK_COMMAND_MATRIX.md",
  "plan_row_count": 480,
  "minimum_plan_row_count": 480,
  "axis_count": 16,
  "defined_research_key_count": 24,
  "used_research_key_count": 24,
  "loc_hotspots": [],
  "claim_drift": [],
  "baseline_gaps": [],
  "innovation_coverage": [],
  "high_risk_vx_linkage": [],
  "stale_doc_markers": [],
  "repo_boundary_findings": [],
  "megakernel_protocol_boundary_findings": [],
  "script_policy_findings": [],
  "rust_toml_loader_findings": [],
  "source_ledger_findings": [],
  "competitor_issue_findings": [],
  "research_plan_coverage_findings": [],
  "archive_replay_findings": [
    {{
      "audit_path": "audits/PHASE10_DIFF.md",
      "line": 42,
      "archived_reference": "vyre-runtime/src/cache.rs",
      "current_lookup": "file-present;symbol-replay-required",
      "replay_fixture_id": "archive-replay:phase10-diff:42:vyre-runtime-src-cache-rs",
      "blocker_status": "replay-required",
      "stale_reason": "archived reference resolves to a current file but needs symbol-level replay before import"
    }}
  ],
  "rules_as_data_findings": [],
  "raw_counter_families": {raw_counter_families},
  "blockers": [],
  "source_digest": "{RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX}abc"
}}"#
        );

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("archive-replay finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_research_plan_coverage_findings() {
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = format!(
            r#"{{
  "schema_version": 6,
  "generator_command": "{RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND}",
  "plan_path": "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md",
  "command_matrix_path": "docs/optimization/XTASK_COMMAND_MATRIX.md",
  "plan_row_count": 480,
  "minimum_plan_row_count": 480,
  "axis_count": 16,
  "defined_research_key_count": 24,
  "used_research_key_count": 24,
  "loc_hotspots": [],
  "claim_drift": [],
  "baseline_gaps": [],
  "innovation_coverage": [],
  "high_risk_vx_linkage": [],
  "stale_doc_markers": [],
  "repo_boundary_findings": [],
  "megakernel_protocol_boundary_findings": [],
  "script_policy_findings": [],
  "rust_toml_loader_findings": [],
  "source_ledger_findings": [],
  "competitor_issue_findings": [],
  "research_plan_coverage_findings": [
    {{
      "path": "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md",
      "key": "VX-999",
      "text": "line 42: VX row lacks rooted local evidence path or explicit active-plan evidence",
      "policy": "research-plan-row-local-evidence"
    }}
  ],
  "archive_replay_findings": [],
  "rules_as_data_findings": [],
  "raw_counter_families": {raw_counter_families},
  "blockers": [],
  "source_digest": "{RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX}abc"
}}"#
        );

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("research-plan coverage finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_rules_as_data_findings() {
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = format!(
            r#"{{
  "schema_version": 6,
  "generator_command": "{RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND}",
  "plan_path": "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md",
  "command_matrix_path": "docs/optimization/XTASK_COMMAND_MATRIX.md",
  "plan_row_count": 480,
  "minimum_plan_row_count": 480,
  "axis_count": 16,
  "defined_research_key_count": 24,
  "used_research_key_count": 24,
  "loc_hotspots": [],
  "claim_drift": [],
  "baseline_gaps": [],
  "innovation_coverage": [],
  "high_risk_vx_linkage": [],
  "stale_doc_markers": [],
  "repo_boundary_findings": [],
  "megakernel_protocol_boundary_findings": [],
  "script_policy_findings": [],
  "rust_toml_loader_findings": [],
  "source_ledger_findings": [],
  "competitor_issue_findings": [],
  "research_plan_coverage_findings": [],
  "archive_replay_findings": [],
  "rules_as_data_findings": [
    {{
      "path": "docs/optimization/RULES_AS_DATA_MANIFEST.toml",
      "key": "fixture-rules",
      "text": "docs/optimization/XTASK_COMMAND_MATRIX.md does not list `docs/optimization/RULES.toml` as a shared source",
      "policy": "rules-as-data-command-matrix-source"
    }}
  ],
  "raw_counter_families": {raw_counter_families},
  "blockers": [],
  "source_digest": "{RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX}abc"
}}"#
        );

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("rules-as-data finding")));
    }

    #[test]
    fn research_audit_artifact_rejects_missing_identity_counter_and_minimum_row_metadata() {
        let raw_counter_families = complete_raw_counter_families_json();
        let artifact = format!(
            r#"{{
  "schema_version": 6,
  "generator_command": "xtask research-audit --output release/evidence/optimization/research-audit.json",
  "plan_row_count": 480,
  "axis_count": 0,
  "defined_research_key_count": 0,
  "used_research_key_count": 0,
  "loc_hotspots": [],
  "claim_drift": [],
  "baseline_gaps": [],
  "innovation_coverage": [],
  "high_risk_vx_linkage": [],
  "stale_doc_markers": [],
  "repo_boundary_findings": [],
  "megakernel_protocol_boundary_findings": [],
  "script_policy_findings": [],
  "rust_toml_loader_findings": [],
  "source_ledger_findings": [],
  "competitor_issue_findings": [],
  "research_plan_coverage_findings": [],
  "archive_replay_findings": [],
  "rules_as_data_findings": [],
  "raw_counter_families": {raw_counter_families},
  "blockers": [],
  "source_digest": "{RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX}abc"
}}"#
        );

        let blockers = validate_research_audit_artifact_bytes(
            artifact.as_bytes(),
            RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND,
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("plan_path")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("command_matrix_path")));
        assert!(blockers.iter().any(|blocker| blocker.contains("axis_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("defined_research_key_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("used_research_key_count")));
    }
}
