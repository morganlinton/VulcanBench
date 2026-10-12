use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(requirement, base_dir, "metadata-matrix.json", failures)
    else {
        return;
    };
    let packages = matrix
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if packages == 0 {
        failures.push("requirement `crate-metadata` matrix contains zero packages".to_string());
    }
    if matrix
        .get("root_patch_section_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX)
        != 0
    {
        failures.push(
            "requirement `crate-metadata` matrix must report zero root [patch.crates-io] sections"
                .to_string(),
        );
    }
    for (field, label) in [
        ("publishable_package_count", "publishable package"),
        ("vyre_package_count", "Vyre package"),
        ("weir_package_count", "Weir package"),
        (
            "parser_release_surface_count",
            "parser release-surface package",
        ),
        (
            "non_publishable_release_surface_count",
            "non-publishable release-surface package",
        ),
    ] {
        if matrix
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            failures.push(format!(
                "requirement `crate-metadata` matrix contains zero {label}(s)"
            ));
        }
    }
    if matrix
        .get("parser_release_surface_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        < 2
    {
        failures.push(
            "requirement `crate-metadata` matrix must include both parser release surfaces: vyrec and vyre-frontend-c"
                .to_string(),
        );
    }
    let missing_required = matrix
        .get("missing_required_release_surfaces")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required != 0 {
        failures.push(format!(
            "requirement `crate-metadata` matrix has {missing_required} missing required release surface(s)"
        ));
    }
    if let Some(entries) = matrix.get("packages").and_then(serde_json::Value::as_array) {
        if !entries.iter().any(|entry| {
            entry.get("name").and_then(serde_json::Value::as_str) == Some("vyrec")
                && entry.get("version").and_then(serde_json::Value::as_str)
                    == Some(crate::release_train::vyrec_version())
                && entry.get("readme").and_then(serde_json::Value::as_str) == Some("README.md")
                && entry
                    .get("release_surface")
                    .and_then(serde_json::Value::as_str)
                    == Some("parser-cli")
        }) {
            failures.push(
                format!(
                    "requirement `crate-metadata` matrix must include vyrec {} parser-cli with README metadata",
                    crate::release_train::vyrec_version()
                ),
            );
        }
        if !entries.iter().any(|entry| {
            entry.get("name").and_then(serde_json::Value::as_str) == Some("vyre-frontend-c")
                && entry.get("version").and_then(serde_json::Value::as_str)
                    == Some(crate::release_train::vyre_frontend_c_version())
                && entry.get("readme").and_then(serde_json::Value::as_str) == Some("README.md")
                && entry
                    .get("release_kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("non-publishable-release-surface")
                && entry
                    .get("release_surface")
                    .and_then(serde_json::Value::as_str)
                    == Some("c-frontend")
        }) {
            failures.push(
                format!(
                    "requirement `crate-metadata` matrix must include vyre-frontend-c {} as a c-frontend non-publishable release surface with README metadata",
                    crate::release_train::vyre_frontend_c_version()
                ),
            );
        }
        for (package_name, backend_surface) in [
            ("vyre-driver-cuda", "cuda-backend"),
            ("vyre-driver-wgpu", "wgpu-backend"),
        ] {
            if !entries.iter().any(|entry| {
                entry.get("name").and_then(serde_json::Value::as_str) == Some(package_name)
                    && entry.get("version").and_then(serde_json::Value::as_str)
                        == Some(crate::release_train::vyre_version())
                    && entry.get("readme").and_then(serde_json::Value::as_str) == Some("README.md")
                    && entry
                        .get("release_kind")
                        .and_then(serde_json::Value::as_str)
                        == Some("publishable-crate")
                    && entry
                        .get("release_surface")
                        .and_then(serde_json::Value::as_str)
                        == Some(backend_surface)
            }) {
                failures.push(format!(
                    "requirement `crate-metadata` matrix must include {package_name} {} as a publishable {backend_surface} release surface with README metadata",
                    crate::release_train::vyre_version()
                ));
            }
        }
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            let release_kind = entry
                .get("release_kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if release_kind == "internal-tooling" {
                continue;
            }
            let release_group = entry
                .get("release_group")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let expected = entry
                .get("expected_version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let version = entry
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if expected.is_empty() || version != expected {
                failures.push(format!(
                    "requirement `crate-metadata` package `{name}` release_group `{release_group}` has version `{version}`, expected `{expected}`"
                ));
            }
            if entry
                .get("example_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `crate-metadata` release package `{name}` has zero examples or README usage blocks"
                ));
            }
            if release_kind == "publishable-crate"
                && entry
                    .get("has_runnable_example")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
            {
                failures.push(format!(
                    "requirement `crate-metadata` publishable release package `{name}` has no runnable examples/*.rs"
                ));
            }
            if release_kind == "publishable-crate"
                && entry
                    .get("has_api_referencing_example")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
            {
                failures.push(format!(
                    "requirement `crate-metadata` publishable release package `{name}` has no API-referencing examples/*.rs"
                ));
            }
        }
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `crate-metadata` matrix still reports {blockers} blocker(s)"
        ));
    }
    let Some(features) =
        first_json_evidence(requirement, base_dir, "feature-matrix.json", failures)
    else {
        return;
    };
    let feature_packages = features
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let feature_blockers = features
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if feature_packages == 0 {
        failures
            .push("requirement `crate-metadata` feature matrix contains zero packages".to_string());
    }
    if feature_blockers != 0 {
        failures.push(format!(
            "requirement `crate-metadata` feature matrix still reports {feature_blockers} blocker(s)"
        ));
    }
    let missing_required_feature_packages = features
        .get("missing_required_release_packages")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_feature_packages != 0 {
        failures.push(format!(
            "requirement `crate-metadata` feature matrix has {missing_required_feature_packages} missing required release package(s)"
        ));
    }
    let feature_entries = features
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (package, required_features) in [
        ("vyre", &["cuda", "wgpu"][..]),
        ("vyre-driver-cuda", &["cuda"][..]),
        ("vyre-driver-wgpu", &["wgpu"][..]),
        ("weir", &["default", "serde"][..]),
    ] {
        let Some(entry) = feature_entries
            .iter()
            .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some(package))
        else {
            failures.push(format!(
                "requirement `crate-metadata` feature matrix is missing `{package}`"
            ));
            continue;
        };
        let features = entry
            .get("features")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for required in required_features {
            if !features
                .iter()
                .any(|feature| feature.as_str() == Some(required))
            {
                failures.push(format!(
                    "requirement `crate-metadata` package `{package}` is missing feature `{required}`"
                ));
            }
        }
    }
    if !feature_entries
        .iter()
        .any(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some("vyrec"))
    {
        failures.push("requirement `crate-metadata` feature matrix is missing `vyrec`".to_string());
    }
    for package in ["vyre", "vyre-driver-cuda", "vyre-driver-wgpu"] {
        let Some(entry) = feature_entries
            .iter()
            .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some(package))
        else {
            failures.push(format!(
                "requirement `crate-metadata` feature matrix is missing `{package}`"
            ));
            continue;
        };
        if entry
            .get("default_feature_members")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|members| !members.is_empty())
        {
            failures.push(format!(
                "requirement `crate-metadata` package `{package}` default feature set must be empty"
            ));
        }
    }
    let Some(package_readiness) =
        first_json_evidence(requirement, base_dir, "publish-readiness.json", failures)
    else {
        return;
    };
    let package_blockers = package_readiness
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if package_blockers != 0 {
        failures.push(format!(
            "requirement `crate-metadata` package readiness still reports {package_blockers} blocker(s)"
        ));
    }
    if package_readiness
        .get("release_train")
        .and_then(|train| train.get("cuda_release_path"))
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        failures.push(
            "requirement `crate-metadata` package readiness must mark CUDA as the release path"
                .to_string(),
        );
    }
    let publish_order = package_readiness
        .get("publish_order")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if publish_order.len() < 20 {
        failures.push(format!(
            "requirement `crate-metadata` package readiness publish_order has only {} package(s)",
            publish_order.len()
        ));
    }
    for required in [
        "vyre-macros",
        "vyre-spec",
        "vyre-foundation",
        "vyre-driver-cuda",
        "vyre-driver-wgpu",
        "vyre",
        "vyre-harness",
        "weir",
        "vyre-libs",
    ] {
        if !publish_order
            .iter()
            .any(|entry| entry.get("package").and_then(serde_json::Value::as_str) == Some(required))
        {
            failures.push(format!(
                "requirement `crate-metadata` package readiness publish_order is missing `{required}`"
            ));
        }
    }
    for field in ["missing_metadata_packages", "extra_metadata_packages"] {
        let count = package_readiness
            .get(field)
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if count != 0 {
            failures.push(format!(
                "requirement `crate-metadata` package readiness field `{field}` has {count} entrie(s)"
            ));
        }
    }
    for field in ["dependency_order_edges", "versioned_local_dependencies"] {
        if package_readiness
            .get(field)
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            failures.push(format!(
                "requirement `crate-metadata` package readiness field `{field}` is empty"
            ));
        }
    }
}
