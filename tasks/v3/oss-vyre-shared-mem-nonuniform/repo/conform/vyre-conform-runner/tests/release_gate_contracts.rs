//! Release-gate contract tests for conform wiring.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

const RUNTIME_DIALECT_CONTRACT_OPS: &[&str] = &[
    "core.indirect_dispatch",
    "io.dma_from_nvme",
    "io.write_back_to_nvme",
    "mem.unmap",
    "mem.zerocopy_map",
];

fn repo_file(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Fix: vyre-conform-runner must stay under the repository conform directory.");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|error| {
        panic!("Fix: expected release-gate file `{path}` to exist: {error}")
    })
}

fn repo_json(path: &str) -> Value {
    serde_json::from_str(&repo_file(path)).unwrap_or_else(|error| {
        panic!("Fix: release artifact `{path}` must be valid JSON: {error}")
    })
}

fn floor(script: &str, crate_name: &str) -> u32 {
    let needle = format!("FLOOR[\"{crate_name}\"]=");
    let line = script
        .lines()
        .find(|line| line.trim_start().starts_with(&needle))
        .unwrap_or_else(|| {
            panic!("Fix: `{crate_name}` must have an explicit test-coverage floor.")
        });
    let value = line[needle.len()..]
        .split(|ch: char| !ch.is_ascii_digit())
        .next()
        .expect("Fix: coverage floor must start with a decimal percentage.");
    value.parse().unwrap_or_else(|error| {
        panic!("Fix: coverage floor for `{crate_name}` must parse: {error}")
    })
}

fn assert_conformance_artifact_has_no_failures(json: &Value, label: &str) {
    let total_pairs = json_usize(json, "total_pairs", label)
        .unwrap_or_else(|| panic!("Fix: `{label}` must define total_pairs."));
    let passed_pairs = json_usize(json, "passed_pairs", label)
        .unwrap_or_else(|| panic!("Fix: `{label}` must define passed_pairs."));
    let failed_pairs = json_usize(json, "failed_pairs", label)
        .unwrap_or_else(|| panic!("Fix: `{label}` must define failed_pairs."));
    assert_eq!(
        failed_pairs, 0,
        "Fix: `{label}` must not ship a release conformance artifact with failing pairs."
    );
    assert_eq!(
        passed_pairs, total_pairs,
        "Fix: `{label}` passed_pairs must equal total_pairs."
    );
    assert!(
        json["blockers"].as_array().is_some_and(Vec::is_empty),
        "Fix: `{label}` must not ship release conformance blockers."
    );
    let pairs = json["pairs"]
        .as_array()
        .unwrap_or_else(|| panic!("Fix: `{label}` must include conformance pairs."));
    assert_eq!(
        pairs.len(),
        total_pairs,
        "Fix: `{label}` total_pairs must match the pairs array length."
    );
    for pair in pairs {
        assert_eq!(
            pair["passed"], true,
            "Fix: `{label}` pair ({:?}, {:?}) must pass before release evidence is accepted.",
            pair["backend_id"], pair["op_id"]
        );
    }
    let diff_summary_count = json_usize(json, "diff_summary_count", label)
        .unwrap_or_else(|| panic!("Fix: `{label}` must define diff_summary_count."));
    let diff_summaries = json["diff_summaries"]
        .as_array()
        .unwrap_or_else(|| panic!("Fix: `{label}` must include diff_summaries."));
    assert_eq!(
        diff_summary_count, total_pairs,
        "Fix: `{label}` diff_summary_count must equal total_pairs."
    );
    assert_eq!(
        diff_summaries.len(),
        total_pairs,
        "Fix: `{label}` diff_summaries length must equal total_pairs."
    );
    let pair_ops = pairs
        .iter()
        .map(|pair| {
            pair["op_id"]
                .as_str()
                .unwrap_or_else(|| panic!("Fix: `{label}` pair must include op_id."))
        })
        .collect::<BTreeSet<_>>();
    let diff_ops = diff_summaries
        .iter()
        .map(|summary| {
            for field in [
                "op_id",
                "backend_id",
                "input_digest",
                "output_digest",
                "timing_class",
                "failure_class",
            ] {
                assert!(
                    summary[field]
                        .as_str()
                        .is_some_and(|value| !value.trim().is_empty()),
                    "Fix: `{label}` diff summary must include non-empty `{field}`."
                );
            }
            summary["op_id"]
                .as_str()
                .expect("Fix: checked above")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        diff_ops, pair_ops,
        "Fix: `{label}` diff_summaries must cover exactly pairs[].op_id."
    );
}

fn assert_runtime_dialect_rows(json: &Value, backend_id: &str, label: &str) {
    let rows = release_backend_rows(json, label);
    let matrix_backend_id = match backend_id {
        "cpu-ref" => "reference",
        other => other,
    };
    let expected_status = match matrix_backend_id {
        "reference" => "not_applicable",
        "cuda" | "wgpu" => "experimental",
        other => panic!("Fix: unknown release backend `{other}` in `{label}`."),
    };
    for op in RUNTIME_DIALECT_CONTRACT_OPS {
        let row = format!("{op}:{matrix_backend_id}:{expected_status}");
        assert!(
            rows.contains(&row),
            "Fix: `{label}` must include runtime dialect release row `{row}`."
        );
    }
}

struct ConformanceSummary {
    distinct_op_count: usize,
    catalog_required_op_count: usize,
    catalog_covered_op_count: usize,
    release_backend_row_count: usize,
    missing_catalog_ops: Vec<String>,
}

impl ConformanceSummary {
    fn from_json(json: &Value, label: &str) -> Self {
        let schema_version = json_usize(json, "schema_version", label)
            .unwrap_or_else(|| panic!("Fix: `{label}` must define schema_version."));
        assert!(
            schema_version >= 3,
            "Fix: `{label}` must use conformance evidence schema v3 or newer."
        );
        let total_pairs = json_usize(json, "total_pairs", label).unwrap_or_else(|| {
            json_usize(json, "op_count", label)
                .unwrap_or_else(|| panic!("Fix: `{label}` must define total_pairs or op_count."))
        });
        let distinct_op_count = json_usize(json, "distinct_op_count", label)
            .unwrap_or_else(|| panic!("Fix: `{label}` must define distinct_op_count."));
        assert_eq!(
            total_pairs, distinct_op_count,
            "Fix: `{label}` must not contain duplicate op-pair certificates."
        );
        Self {
            distinct_op_count,
            catalog_required_op_count: json_usize(json, "catalog_required_op_count", label)
                .unwrap_or_else(|| panic!("Fix: `{label}` must define catalog_required_op_count.")),
            catalog_covered_op_count: json_usize(json, "catalog_covered_op_count", label)
                .unwrap_or_else(|| panic!("Fix: `{label}` must define catalog_covered_op_count.")),
            release_backend_row_count: json_usize(json, "release_backend_row_count", label)
                .unwrap_or_else(|| panic!("Fix: `{label}` must define release_backend_row_count.")),
            missing_catalog_ops: json["missing_catalog_ops"]
                .as_array()
                .unwrap_or_else(|| panic!("Fix: `{label}` must define missing_catalog_ops."))
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("Fix: `{label}` missing_catalog_ops entries must be strings.")
                        })
                        .to_string()
                })
                .collect(),
        }
    }
}

fn json_usize(json: &Value, key: &str, label: &str) -> Option<usize> {
    json[key].as_u64().map(|value| {
        usize::try_from(value).unwrap_or_else(|error| {
            panic!("Fix: `{label}` field `{key}` cannot fit usize: {error}")
        })
    })
}

fn release_backend_rows(json: &Value, label: &str) -> BTreeSet<String> {
    json["release_backend_rows"]
        .as_array()
        .unwrap_or_else(|| panic!("Fix: `{label}` must define release_backend_rows."))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| {
                    panic!("Fix: `{label}` release_backend_rows entries must be strings.")
                })
                .to_string()
        })
        .collect()
}

fn assert_complete_backend_rows(rows: &BTreeSet<String>, expected_ops_per_backend: usize) {
    let mut per_backend: BTreeMap<&str, usize> = BTreeMap::new();
    let mut ops = BTreeSet::new();
    let mut runtime_status_rows = BTreeSet::new();
    for row in rows {
        let (op, backend, status) = parse_release_backend_row(row);
        assert!(
            matches!(backend, "reference" | "cuda" | "wgpu"),
            "Fix: release conformance row `{row}` has unexpected backend `{backend}`."
        );
        if RUNTIME_DIALECT_CONTRACT_OPS.contains(&op) {
            let expected = if backend == "reference" {
                "not_applicable"
            } else {
                "experimental"
            };
            assert_eq!(
                status, expected,
                "Fix: runtime dialect contract row `{row}` must use status `{expected}` until a concrete backend lowering is promoted."
            );
            runtime_status_rows.insert(row.clone());
        } else {
            assert_eq!(
                status, "supported",
                "Fix: non-runtime release conformance row `{row}` must be supported."
            );
        }
        *per_backend.entry(backend).or_default() += 1;
        ops.insert(op.to_string());
    }
    assert_eq!(
        runtime_status_rows.len(),
        RUNTIME_DIALECT_CONTRACT_OPS.len() * 3,
        "Fix: runtime dialect exceptions must be explicit and limited to the Category C runtime contract ops."
    );
    for backend in ["reference", "cuda", "wgpu"] {
        assert_eq!(
            per_backend.get(backend).copied().unwrap_or(0),
            expected_ops_per_backend,
            "Fix: release conformance must contain one `{backend}` row for every required catalog op."
        );
    }
    assert_eq!(
        ops.len(),
        expected_ops_per_backend,
        "Fix: release conformance row set must contain exactly the required catalog op set."
    );
}

fn parse_release_backend_row(row: &str) -> (&str, &str, &str) {
    let (prefix, status) = row
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("Fix: release backend row `{row}` must include a status."));
    let (op, backend) = prefix
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("Fix: release backend row `{row}` must include a backend."));
    (op, backend, status)
}

fn assert_json_string_array_contains_exactly(json: &Value, expected: &[&str], field: &str) {
    let actual = json
        .as_array()
        .unwrap_or_else(|| panic!("Fix: `{field}` must be an array."))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("Fix: `{field}` entries must be strings."))
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|value| (*value).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "Fix: `{field}` has the wrong entries.");
}

fn shell_scripts_under(root: PathBuf) -> Vec<PathBuf> {
    let mut scripts = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|error| {
            panic!(
                "Fix: script directory `{}` must be readable: {error}",
                dir.display()
            )
        }) {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!("Fix: script directory entry must be readable: {error}")
                })
                .path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "sh") {
                scripts.push(path);
            }
        }
    }
    scripts.sort();
    scripts
}

fn scan_for_raw_backend_factory_calls(root: &Path, path: &Path, findings: &mut Vec<String>) {
    let entries = std::fs::read_dir(path).unwrap_or_else(|error| {
        panic!(
            "Fix: expected test directory `{}` to be readable: {error}",
            path.display()
        )
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "Fix: expected directory entry under `{}` to be readable: {error}",
                path.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!(
                "Fix: expected `{}` metadata to be readable: {error}",
                path.display()
            )
        });
        if file_type.is_dir() {
            scan_for_raw_backend_factory_calls(root, &path, findings);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("Fix: expected `{}` to be readable: {error}", path.display())
        });
        let compact = source
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        let member_factory_call = [".", "factory", ")()"].concat();
        let registration_factory_call = ["(", "registration", ".", "factory", ")()"].concat();
        if compact.contains(&member_factory_call) || compact.contains(&registration_factory_call) {
            let relative = path.strip_prefix(root).unwrap_or(path.as_path());
            findings.push(relative.display().to_string());
        }
    }
}

fn scan_for_cfg_gated_gpu_tests(root: &Path, path: &Path, findings: &mut Vec<String>) {
    let entries = std::fs::read_dir(path).unwrap_or_else(|error| {
        panic!(
            "Fix: expected test directory `{}` to be readable: {error}",
            path.display()
        )
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "Fix: expected directory entry under `{}` to be readable: {error}",
                path.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!(
                "Fix: expected `{}` metadata to be readable: {error}",
                path.display()
            )
        });
        if file_type.is_dir() {
            scan_for_cfg_gated_gpu_tests(root, &path, findings);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("Fix: expected `{}` to be readable: {error}", path.display())
        });
        let mut lines = source.lines().enumerate().peekable();
        while let Some((index, line)) = lines.next() {
            if !compacted_eq(line, "#[cfg(feature=\"gpu\")]") {
                continue;
            }
            let mut next = "";
            while let Some((_, candidate)) = lines.peek() {
                let trimmed = candidate.trim();
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    lines.next();
                    continue;
                }
                next = *candidate;
                break;
            }
            if compacted_eq(next, "#[test]") || compacted_starts_with(next, "mod") {
                let relative = path.strip_prefix(root).unwrap_or(path.as_path());
                findings.push(format!("{}:{}", relative.display(), index + 1));
            }
        }
    }
}

fn compacted_eq(input: &str, expected: &str) -> bool {
    let mut expected = expected.chars();
    for ch in input.chars().filter(|ch| !ch.is_whitespace()) {
        match expected.next() {
            Some(expected_ch) if expected_ch == ch => {}
            _ => return false,
        }
    }
    expected.next().is_none()
}

fn compacted_starts_with(input: &str, expected: &str) -> bool {
    let mut expected = expected.chars();
    for ch in input.chars().filter(|ch| !ch.is_whitespace()) {
        match expected.next() {
            Some(expected_ch) if expected_ch == ch => {}
            Some(_) => return false,
            None => return true,
        }
    }
    expected.next().is_none()
}

fn concrete_driver_crates() -> Vec<String> {
    let manifest = repo_file("Cargo.toml");
    // The workspace `members` section uses bare quoted strings like
    // `"vyre-driver-wgpu",`. The `[workspace.dependencies]` table
    // uses the same prefix in lines like
    // `vyre-driver-wgpu = { version = ... }`  -  those must NOT match
    // here, otherwise the whole dep line gets treated as a crate name.
    // Restrict to lines whose trimmed-of-quotes/commas form is a bare
    // crate name (no spaces, no `=`).
    manifest
        .lines()
        .filter_map(|line| {
            let member = line.trim().trim_matches(',').trim_matches('"');
            (member.starts_with("vyre-driver-") && !member.contains(' ') && !member.contains('='))
                .then(|| member.to_string())
        })
        .collect()
}

#[path = "release_gate_contracts/artifact_catalog_contracts.rs"]
mod artifact_catalog_contracts;
#[path = "release_gate_contracts/driver_floor_contracts.rs"]
mod driver_floor_contracts;
#[path = "release_gate_contracts/ci_script_contracts.rs"]
mod ci_script_contracts;
#[path = "release_gate_contracts/backend_acquisition_contracts.rs"]
mod backend_acquisition_contracts;
#[path = "release_gate_contracts/planner_contracts.rs"]
mod planner_contracts;
#[path = "release_gate_contracts/static_scan_contracts.rs"]
mod static_scan_contracts;
