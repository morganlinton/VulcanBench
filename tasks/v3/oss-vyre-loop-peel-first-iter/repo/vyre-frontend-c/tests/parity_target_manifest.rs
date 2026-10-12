//! Contract tests for the frozen Linux lib/math parity target manifest.

use std::collections::BTreeSet;

const TARGET_MANIFEST: &str = include_str!("../parity/linux_math_v6_8.toml");

fn target_manifest() -> toml::Value {
    toml::from_str(TARGET_MANIFEST).expect("linux_math_v6_8.toml must be valid TOML")
}

#[test]
fn linux_math_v6_8_manifest_freezes_release_target() {
    let manifest = target_manifest();

    assert_eq!(string(&manifest, "schema"), "vyrec.parity.target.v1");
    assert_eq!(string(&manifest, "id"), "linux-lib-math-v6.8");
    assert_eq!(
        string(&manifest, "commit"),
        "90d1f30371ae3337beb01666b226320728d35c70"
    );
    assert_eq!(string(&manifest, "subsystem_root"), "lib/math");
    assert_eq!(string(&manifest, "language"), "gnu11");

    let files = table(&manifest, "files");
    let sources = string_array_value(files, "sources");
    assert_eq!(
        sources.len(),
        12,
        "Linux lib/math target must cover every C translation unit frozen for v6.8"
    );
    assert!(sources.iter().all(|path| path.starts_with("lib/math/")));
    assert!(sources.iter().all(|path| path.ends_with(".c")));

    let translation_units = manifest
        .get("translation_units")
        .and_then(toml::Value::as_array)
        .expect("translation_units must be an array");
    assert_eq!(translation_units.len(), sources.len());

    let source_set = sources.iter().cloned().collect::<BTreeSet<_>>();
    let tu_set = translation_units
        .iter()
        .map(|entry| string(entry, "path"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        tu_set, source_set,
        "translation_units must exactly match sources"
    );

    let direct_headers = string_array_value(files, "direct_headers")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert!(
        direct_headers.contains("kunit/test.h"),
        "target must include the KUnit translation unit, not only production-looking files"
    );
    assert!(
        direct_headers.contains("asm/div64.h"),
        "target must include asm headers encountered by the real subsystem"
    );
    assert!(
        direct_headers.contains("linux/reciprocal_div.h"),
        "target must include subsystem-specific kernel headers"
    );

    let mut referenced_headers = BTreeSet::new();
    for entry in translation_units {
        let path = string(entry, "path");
        let headers = string_array(entry, "direct_headers");
        assert!(!headers.is_empty(), "{path} must record direct includes");
        for header in headers {
            referenced_headers.insert(header);
        }
    }
    assert_eq!(
        referenced_headers, direct_headers,
        "direct_headers must be the exact union of translation-unit headers"
    );
}

#[test]
fn linux_math_v6_8_manifest_requires_gpu_parity_and_performance_proof() {
    let manifest = target_manifest();

    let scope = table(&manifest, "scope");
    assert_eq!(
        scope.get("lowering").and_then(toml::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        scope
            .get("pre_lowering_required")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        scope
            .get("clang_parity_through")
            .and_then(toml::Value::as_str),
        Some("semantic-analysis")
    );
    assert_eq!(
        scope
            .get("cpu_execution_allowed")
            .and_then(toml::Value::as_str),
        Some("oracle-only")
    );
    assert_eq!(
        scope
            .get("gpu_execution_required")
            .and_then(toml::Value::as_bool),
        Some(true)
    );

    let gates = table(&manifest, "release_gates");
    for gate in [
        "zero_unexplained_parity_mismatches",
        "zero_silent_cpu_fallbacks",
        "zero_false_no_gpu_skips",
        "resident_gpu_frontend_required",
        "megakernel_or_resident_graph_measurement_required",
        "clang_baseline_required",
        "reproducible_performance_claim_required",
    ] {
        assert_eq!(
            gates.get(gate).and_then(toml::Value::as_bool),
            Some(true),
            "release gate {gate} must be enabled"
        );
    }

    let proof = table(&manifest, "proof_required");
    for category in [
        "preprocessor",
        "lexer",
        "parser",
        "semantic_analysis",
        "abi_layout",
        "performance",
    ] {
        let requirements = proof
            .get(category)
            .and_then(toml::Value::as_array)
            .expect("proof_required category must be an array");
        assert!(
            !requirements.is_empty(),
            "proof_required.{category} must not be empty"
        );
    }
}

fn table<'a>(root: &'a toml::Value, key: &str) -> &'a toml::map::Map<String, toml::Value> {
    root.get(key)
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("{key} must be a table"))
}

fn string(root: &toml::Value, key: &str) -> String {
    root.get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("{key} must be a string"))
        .to_owned()
}

fn string_array(root: &toml::Value, key: &str) -> Vec<String> {
    string_array_from(root.get(key), key)
}

fn string_array_value(root: &toml::map::Map<String, toml::Value>, key: &str) -> Vec<String> {
    string_array_from(root.get(key), key)
}

fn string_array_from(value: Option<&toml::Value>, key: &str) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{key} must be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{key} entries must be strings"))
                .to_owned()
        })
        .collect()
}
