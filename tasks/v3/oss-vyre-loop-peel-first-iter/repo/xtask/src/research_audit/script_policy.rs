use std::fs;
use std::path::Path;

use super::collectors::{compact_line, rel_path, skip_path};
use super::model::ScriptPolicyFinding;

pub(super) fn collect_script_policy_findings(root: &Path) -> Vec<ScriptPolicyFinding> {
    let mut findings = Vec::new();
    collect_script_policy_findings_in(root, &root.join("scripts"), &mut findings);
    findings
}

fn collect_script_policy_findings_in(
    root: &Path,
    path: &Path,
    findings: &mut Vec<ScriptPolicyFinding>,
) {
    let rel = rel_path(root, path);
    if skip_path(&rel) {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_script_policy_findings_in(root, &entry.path(), findings);
            }
        }
        return;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("sh") {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    if duplicate_shell_toml_parser_body(&rel, &text) {
        let line = text
            .lines()
            .position(|line| line.contains("tomllib.load"))
            .map_or(1, |index| index + 1);
        findings.push(ScriptPolicyFinding {
            path: rel.clone(),
            line,
            text: compact_line("duplicate shell TOML parser body"),
            policy:
                "use scripts/lib/toml_reader.sh as the only shell TOML parser body".to_string(),
        });
    }
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.contains("eval ") || trimmed.contains("bash -c") || trimmed.contains("sh -c") {
            findings.push(ScriptPolicyFinding {
                path: rel.clone(),
                line: line_index + 1,
                text: compact_line(line),
                policy: "no dynamic shell command construction in release scripts".to_string(),
            });
        }
        if sensitive_release_command(trimmed) && has_unquoted_shell_variable(trimmed) {
            findings.push(ScriptPolicyFinding {
                path: rel.clone(),
                line: line_index + 1,
                text: compact_line(line),
                policy: "quote variables in release-script commands that handle repository targets, branches, tags, or evidence paths".to_string(),
            });
        }
    }
}

fn duplicate_shell_toml_parser_body(rel: &str, text: &str) -> bool {
    rel.starts_with("scripts/lib/")
        && rel != "scripts/lib/toml_reader.sh"
        && text.contains("python3 -")
        && text.contains("import tomllib")
        && text.contains("tomllib.load")
}

fn sensitive_release_command(line: &str) -> bool {
    line.starts_with("git ")
        || line.starts_with("gh ")
        || line.starts_with("cp ")
        || line.starts_with("mkdir ")
        || line.starts_with("jq ")
        || line.contains(" git ")
        || line.contains(" gh ")
}

fn has_unquoted_shell_variable(line: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '$' if !in_single && !in_double => {
                let next = chars.get(index + 1).copied();
                if next.is_some_and(|ch| ch == '{' || ch == '_' || ch.is_ascii_alphabetic()) {
                    return true;
                }
            }
            _ => {}
        }
        index += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::collect_script_policy_findings;

    #[test]
    fn duplicate_shell_toml_parser_body_is_a_script_policy_finding() {
        let dir = tempfile::tempdir().expect("Fix: create script-policy fixture directory.");
        let lib = dir.path().join("scripts/lib");
        std::fs::create_dir_all(&lib).expect("Fix: create scripts/lib fixture directory.");
        std::fs::write(
            lib.join("release_train.sh"),
            r#"#!/usr/bin/env bash
python3 - "$manifest" <<'PY'
import tomllib
tomllib.load(handle)
PY
"#,
        )
        .expect("Fix: write duplicate shell TOML parser fixture.");
        std::fs::write(
            lib.join("toml_reader.sh"),
            r#"#!/usr/bin/env bash
python3 - "$manifest" <<'PY'
import tomllib
tomllib.load(handle)
PY
"#,
        )
        .expect("Fix: write canonical shell TOML parser fixture.");

        let findings = collect_script_policy_findings(dir.path());

        assert!(
            findings.iter().any(|finding| {
                finding.path == "scripts/lib/release_train.sh"
                    && finding
                        .policy
                        .contains("scripts/lib/toml_reader.sh as the only shell TOML parser")
            }),
            "Fix: release script helpers must not copy the shell TOML parser body; findings={findings:?}"
        );
        assert!(
            findings
                .iter()
                .all(|finding| finding.path != "scripts/lib/toml_reader.sh"),
            "Fix: the canonical TOML reader must be allowed to own the parser body; findings={findings:?}"
        );
    }
}
