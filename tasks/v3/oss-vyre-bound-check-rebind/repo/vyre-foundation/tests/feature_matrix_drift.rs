//! Feature matrix drift contract.
//!
//! Features declared in Cargo.toml should have at least one `cfg(feature)`
//! gate in source, unless they are baselined as dependency-only or aggregate
//! roll-ups. New features with zero source references are forbidden.

use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn feature_matrix_drift_is_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    // (crate, known features with zero cfg references in source)
    let crates: &[(&str, &[&str])] = &[
        (
            "vyre-libs",
            &[
                // Aggregate roll-ups
                "default",
                "math",
                "nn",
                "matching",
                "crypto",
                "parsing",
                // Currently unused but declared
                "hardware",
                "composite",
                // Aggregate roll-up + dev-only / migration features
                // Codex's recent restructure surfaced. Tracked under
                // ROADMAP S19; remove from this list when the source
                // gates them or the features are dropped from
                // Cargo.toml.
                "full",
                "self-substrate",
                "test-fixtures",
                "nn-inference",
                "cpu-parity",
                "text",
            ],
        ),
        (
            "vyre-primitives",
            &[
                "default",  // meta-default
                "all-lego", // aggregate
                "cpu-parity",
                "gpu",
            ],
        ),
        (
            "vyre-runtime",
            &[
                "default",
                "subgroup-ops",       // declared but currently ungated in source
                "remote-cache",       // dependency-only (enables ureq)
                "c-frontend-adapter", // adapter surface marker; no source cfg gate
            ],
        ),
        (
            "vyre-intrinsics",
            &[
                "default",
                "all",          // aggregate
                "subgroup-ops", // gated downstream, declared upstream as a marker
            ],
        ),
    ];

    let mut violations = Vec::new();

    for (crate_name, known_zero_cfg) in crates {
        let crate_root = workspace_root.join(crate_name);
        let toml_path = crate_root.join("Cargo.toml");
        if !toml_path.exists() {
            continue;
        }
        let toml_content = std::fs::read_to_string(&toml_path).unwrap();

        // Parse declared feature names from [features] section.
        let mut declared = HashSet::new();
        let mut in_features = false;
        for line in toml_content.lines() {
            let trimmed = line.trim();
            if trimmed == "[features]" {
                in_features = true;
                continue;
            }
            if in_features && trimmed.starts_with('[') && trimmed.ends_with(']') {
                break;
            }
            if in_features && !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some(eq) = trimmed.find('=') {
                    let name = trimmed[..eq].trim().trim_matches('"').to_string();
                    declared.insert(name);
                }
            }
        }

        // Scan src/ for cfg(feature = "...") references.
        let mut found_in_source = HashSet::new();
        let src = crate_root.join("src");
        if src.is_dir() {
            let mut stack = vec![src];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                        let content = std::fs::read_to_string(&path).unwrap();
                        for line in content.lines() {
                            // A feature gate appears as `feature = "NAME"` inside a
                            // `cfg(...)`. The old `cfg(feature = "..."` match only
                            // caught the BARE form and missed every compound cfg —
                            // `cfg(any(test, feature = "x"))`, `cfg(all(...))`,
                            // `cfg(not(...))` — falsely reporting such features as
                            // unused. Scan every `feature = "NAME"` on any cfg line.
                            if !line.contains("cfg") {
                                continue;
                            }
                            for seg in line.split("feature").skip(1) {
                                let seg = seg.trim_start();
                                let Some(seg) = seg.strip_prefix('=') else {
                                    continue;
                                };
                                let seg = seg.trim_start();
                                let Some(seg) = seg.strip_prefix('"') else {
                                    continue;
                                };
                                if let Some(end) = seg.find('"') {
                                    found_in_source.insert(seg[..end].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        let known: HashSet<String> = known_zero_cfg.iter().map(|s| s.to_string()).collect();

        for feat in &declared {
            if !found_in_source.contains(feat) && !known.contains(feat) {
                violations.push(format!(
                    "{}: feature '{}' declared but has zero cfg references in source",
                    crate_name, feat
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "feature matrix drift detected. Baselined exceptions are allowed; new ones must be justified.\n{}",
        violations.join("\n")
    );
}
