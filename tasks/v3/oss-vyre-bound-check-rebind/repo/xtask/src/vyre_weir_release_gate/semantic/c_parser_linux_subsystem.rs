use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    if !requirement.evidence.iter().any(|evidence| {
        evidence.contains("cargo_full")
            && evidence.contains("c-parser-corpus")
            && evidence.contains("--corpus")
            && evidence.contains("-I")
            && evidence.contains("-D")
    }) {
        failures.push(
            "requirement `c-parser-linux-subsystem` must include a cargo_full c-parser-corpus command with --corpus, -I, and -D"
                .to_string(),
        );
    }
    let Some(report) = first_json_evidence(
        requirement,
        base_dir,
        "c-parser-linux-subsystem.json",
        failures,
    ) else {
        return;
    };
    let total = report
        .get("total_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let parsed = report
        .get("parsed_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let failed = report
        .get("failed_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let source_bytes = report
        .get("total_source_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let ast_bytes = report
        .get("total_ast_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let vast_bytes = report
        .get("total_vast_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let semantic_graph_bytes = report
        .get("total_semantic_graph_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let blockers = report
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if total < 250 {
        failures.push(
            format!(
                "requirement `c-parser-linux-subsystem` corpus report contains {total} C file(s), needs at least 250"
            ),
        );
    }
    if source_bytes < 4 * 1024 * 1024 {
        failures.push(format!(
            "requirement `c-parser-linux-subsystem` corpus report contains {source_bytes} source byte(s), needs at least 4194304"
        ));
    }
    if report
        .get("linux_subsystem_candidate")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must prove linux_subsystem_candidate=true"
                .to_string(),
        );
    }
    if report
        .get("corpus_root_canonical")
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must include corpus_root_canonical"
                .to_string(),
        );
    }
    if report
        .get("corpus_fingerprint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|value| !value.starts_with("fnv64:"))
    {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must include stable corpus_fingerprint"
                .to_string(),
        );
    }
    if report
        .get("source_collection_mode")
        .and_then(serde_json::Value::as_str)
        != Some("recursive_all_c_files")
    {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must prove recursive_all_c_files source collection"
                .to_string(),
        );
    }
    if report
        .get("visited_dir_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must prove nonzero recursive directory traversal"
                .to_string(),
        );
    }
    for field in ["linux_root", "linux_subsystem", "linux_kbuild_file"] {
        if report
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` corpus report must include `{field}` provenance"
            ));
        }
    }
    if report
        .get("linux_kbuild_file_in_corpus")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must prove linux_kbuild_file_in_corpus=true"
                .to_string(),
        );
    }
    let linux_subsystem = report
        .get("linux_subsystem")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !matches!(
        linux_subsystem,
        "kernel" | "fs" | "mm" | "net" | "drivers" | "lib"
    ) {
        failures.push(format!(
            "requirement `c-parser-linux-subsystem` corpus report has unsupported linux_subsystem `{linux_subsystem}`"
        ));
    }
    let linux_depth = report
        .get("linux_subsystem_depth")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if linux_depth == 0 {
        failures.push(
            "requirement `c-parser-linux-subsystem` corpus report must prove linux_subsystem_depth > 0"
                .to_string(),
        );
    }
    for field in ["include_dirs", "macros"] {
        if report
            .get(field)
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` corpus report `{field}` must be non-empty"
            ));
        }
    }
    if ast_bytes == 0 || vast_bytes == 0 || semantic_graph_bytes == 0 {
        failures.push(format!(
            "requirement `c-parser-linux-subsystem` AST/VAST/semantic evidence is incomplete: ast_bytes={ast_bytes}, vast_bytes={vast_bytes}, semantic_graph_bytes={semantic_graph_bytes}"
        ));
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `c-parser-linux-subsystem` corpus report still has {blockers} blocker(s)"
        ));
    }
    if failed != 0 || parsed != total {
        failures.push(format!(
            "requirement `c-parser-linux-subsystem` parsed {parsed}/{total} file(s), failed {failed}; release requires full corpus parse"
        ));
    }
    if let Some(manifest) = first_json_evidence(
        requirement,
        base_dir,
        "linux-subsystem-corpus-manifest.json",
        failures,
    ) {
        let manifest_files = manifest
            .get("file_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if manifest_files != total {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` corpus manifest lists {manifest_files} file(s), parse report lists {total}"
            ));
        }
        let manifest_source_bytes = manifest
            .get("total_source_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if manifest_source_bytes != source_bytes {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` corpus manifest lists {manifest_source_bytes} source byte(s), parse report lists {source_bytes}"
            ));
        }
        for field in [
            "include_dirs",
            "macros",
            "corpus_root_canonical",
            "linux_subsystem_candidate",
            "linux_root",
            "linux_subsystem",
            "linux_subsystem_depth",
            "linux_kbuild_file",
            "linux_kbuild_file_in_corpus",
            "corpus_fingerprint",
            "source_collection_mode",
            "visited_dir_count",
        ] {
            if manifest.get(field) != report.get(field) {
                failures.push(format!(
                    "requirement `c-parser-linux-subsystem` corpus manifest `{field}` does not match parse report"
                ));
            }
        }
        let manifest_entries = manifest
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len) as u64;
        if manifest_entries != total {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` corpus manifest has {manifest_entries} file entrie(s), parse report lists {total}"
            ));
        }
        if let Some(files) = manifest.get("files").and_then(serde_json::Value::as_array) {
            for file in files {
                let path = file
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<unknown>");
                if file.get("parsed").and_then(serde_json::Value::as_bool) != Some(true) {
                    failures.push(format!(
                        "requirement `c-parser-linux-subsystem` corpus manifest file `{path}` was not parsed successfully"
                    ));
                    continue;
                }
                for field in [
                    "source_bytes",
                    "object_bytes",
                    "ast_bytes",
                    "vast_bytes",
                    "semantic_graph_bytes",
                    "wall_ns",
                ] {
                    if file
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        failures.push(format!(
                            "requirement `c-parser-linux-subsystem` corpus manifest file `{path}` has zero `{field}`"
                        ));
                    }
                }
            }
        }
    }
    if let Some(diagnostics) = first_json_evidence(
        requirement,
        base_dir,
        "c-parser-diagnostics-summary.json",
        failures,
    ) {
        let diagnostic_failures = diagnostics
            .get("failed_files")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX);
        if diagnostic_failures != failed {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` diagnostics report lists {diagnostic_failures} failure(s), parse report lists {failed}"
            ));
        }
        let diagnostic_entries = diagnostics
            .get("failures")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if failed == 0 && diagnostic_entries != 0 {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` diagnostics report has {diagnostic_entries} failure entrie(s) while parse report lists zero"
            ));
        }
    }
    if let Some(throughput) =
        first_json_evidence(requirement, base_dir, "c-parser-throughput.json", failures)
    {
        let throughput_files = throughput
            .get("parsed_files")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if throughput_files != parsed {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` throughput report lists {throughput_files} parsed file(s), parse report lists {parsed}"
            ));
        }
        let throughput_total = throughput
            .get("total_files")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if throughput_total != total {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` throughput report lists {throughput_total} total file(s), parse report lists {total}"
            ));
        }
        let throughput_source_bytes = throughput
            .get("total_source_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if throughput_source_bytes != source_bytes {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` throughput report lists {throughput_source_bytes} source byte(s), parse report lists {source_bytes}"
            ));
        }
        for field in [
            "include_dirs",
            "macros",
            "corpus_root_canonical",
            "linux_subsystem_candidate",
            "linux_root",
            "linux_subsystem",
            "linux_subsystem_depth",
            "linux_kbuild_file",
            "linux_kbuild_file_in_corpus",
            "corpus_fingerprint",
            "source_collection_mode",
            "visited_dir_count",
        ] {
            if throughput.get(field) != report.get(field) {
                failures.push(format!(
                    "requirement `c-parser-linux-subsystem` throughput report `{field}` does not match parse report"
                ));
            }
        }
        let wall_ns = throughput
            .get("wall_ns")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let files_per_second = throughput
            .get("files_per_second_x1000")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let mib_per_second = throughput
            .get("mib_per_second_x1000")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if wall_ns == 0 || files_per_second == 0 || mib_per_second == 0 {
            failures.push(format!(
                "requirement `c-parser-linux-subsystem` throughput is incomplete: wall_ns={wall_ns}, files_per_second_x1000={files_per_second}, mib_per_second_x1000={mib_per_second}"
            ));
        }
    }
}
