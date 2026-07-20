use std::sync::OnceLock;

use serde::Deserialize;

pub(crate) const REPO_BOUNDARY_TOML_PATH: &str = "release/repo-boundary.toml";
const REPO_BOUNDARY_TOML: &str = include_str!("../../release/repo-boundary.toml");

#[derive(Debug, Deserialize)]
struct RepoBoundaryData {
    public_repository: String,
    private_repository: String,
    public_repository_field: String,
    legacy_public_repositories_field: String,
    legacy_plural_release_repo_variable: String,
    verify_public_repo_action: String,
    boundary_description: String,
}

static REPO_BOUNDARY: OnceLock<Result<RepoBoundaryData, String>> = OnceLock::new();
static VERIFY_PUBLIC_REPO_EVIDENCE: OnceLock<String> = OnceLock::new();

fn data() -> &'static RepoBoundaryData {
    crate::toml_config::data_or_exit(REPO_BOUNDARY.get_or_init(|| {
        crate::toml_config::parse_embedded_toml(REPO_BOUNDARY_TOML_PATH, REPO_BOUNDARY_TOML)
    }))
}

pub(crate) fn vyre_public_repository() -> &'static str {
    data().public_repository.as_str()
}

pub(crate) fn public_repository_field() -> &'static str {
    data().public_repository_field.as_str()
}

pub(crate) fn legacy_public_repositories_field() -> &'static str {
    data().legacy_public_repositories_field.as_str()
}

pub(crate) fn verify_public_repo_action() -> &'static str {
    data().verify_public_repo_action.as_str()
}

pub(crate) fn verify_public_repo_evidence() -> &'static str {
    VERIFY_PUBLIC_REPO_EVIDENCE
        .get_or_init(|| {
            format!(
                "scripts/final-launch.sh + gh repo view {} --json visibility",
                vyre_public_repository()
            )
        })
        .as_str()
}

pub(crate) fn repo_boundary_description() -> &'static str {
    data().boundary_description.as_str()
}

pub(crate) fn has_single_public_repository(value: &serde_json::Value) -> bool {
    value.get(legacy_public_repositories_field()).is_none()
        && value
        .get(public_repository_field())
        .and_then(serde_json::Value::as_str)
        == Some(vyre_public_repository())
}

pub(crate) fn public_repository_field_is_singular(value: &serde_json::Value) -> bool {
    value.get(legacy_public_repositories_field()).is_none()
}

pub(crate) fn touches_private_santh_visibility(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains(&data().private_repository.to_ascii_lowercase())
        || (lower.contains("gh repo edit") && lower.contains("santh"))
        || line.contains(data().legacy_plural_release_repo_variable.as_str())
}

pub(crate) fn public_artifact_boundary_blockers(artifact: &str, bytes: &[u8]) -> Vec<String> {
    let mut blockers = Vec::new();
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => inspect_public_artifact_json(artifact, "$", &value, &mut blockers),
        Err(_) => {
            let text = String::from_utf8_lossy(bytes);
            inspect_public_artifact_text(artifact, &text, true, &mut blockers);
        }
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

fn inspect_public_artifact_text(
    artifact: &str,
    text: &str,
    command_sensitive: bool,
    blockers: &mut Vec<String>,
) {
    let lower = text.to_ascii_lowercase();
    if contains_private_santh_path(&lower) {
        blockers.push(format!(
            "{artifact}: public artifact contains a private Santh path"
        ));
    }
    if contains_credential_marker(&lower) {
        blockers.push(format!(
            "{artifact}: public artifact contains credential-looking provenance"
        ));
    }
    if command_sensitive && lower.contains("gh repo edit") && lower.contains("santh") {
        blockers.push(format!(
            "{artifact}: public artifact contains private Santh visibility mutation command"
        ));
    }
    if command_sensitive && text.contains(data().legacy_plural_release_repo_variable.as_str()) {
        blockers.push(format!(
            "{artifact}: public artifact contains legacy plural release repo variable `{}`",
            data().legacy_plural_release_repo_variable.as_str()
        ));
    }
    for repo in github_repo_refs(&lower) {
        if repo != vyre_public_repository().to_ascii_lowercase() {
            blockers.push(format!(
                "{artifact}: public artifact names non-Vyre public repository `{repo}`"
            ));
        }
    }
}

fn inspect_public_artifact_json(
    artifact: &str,
    path: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let child_path = format!("{path}.{key}");
                if key == legacy_public_repositories_field() {
                    blockers.push(format!(
                        "{artifact}: public artifact field `{child_path}` must use singular `{}`",
                        public_repository_field()
                    ));
                }
                if key == public_repository_field()
                    && value.as_str() != Some(vyre_public_repository())
                {
                    blockers.push(format!(
                        "{artifact}: public artifact field `{child_path}` must be `{}`",
                        vyre_public_repository()
                    ));
                }
                if let serde_json::Value::String(text) = value {
                    inspect_public_artifact_text(
                        artifact,
                        text,
                        command_sensitive_json_path(&child_path),
                        blockers,
                    );
                }
                inspect_public_artifact_json(artifact, &child_path, value, blockers);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                inspect_public_artifact_json(
                    artifact,
                    &format!("{path}[{index}]"),
                    value,
                    blockers,
                );
            }
        }
        serde_json::Value::String(text) => {
            inspect_public_artifact_text(
                artifact,
                text,
                command_sensitive_json_path(path),
                blockers,
            );
        }
        _ => {}
    }
}

fn command_sensitive_json_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        "action",
        "command",
        "env",
        "environment",
        "provenance",
        "script",
        "shell",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn contains_private_santh_path(lower: &str) -> bool {
    lower.contains("/santh/")
        || lower.contains("\\santh\\")
        || lower.contains("santhdata/santh")
        || lower.contains("santhdata\\santh")
        || lower.contains("santhsecurity/santh")
}

fn contains_credential_marker(lower: &str) -> bool {
    [
        "authorization: bearer",
        "token=",
        "password=",
        "secret=",
        "api_key=",
        "apikey=",
        "github_pat_",
        "ghp_",
        "/credentials/",
        "\\credentials\\",
        "c:\\credentials\\",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn github_repo_refs(lower: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut remaining = lower;
    while let Some(offset) = remaining.find("santhsecurity/") {
        let after = &remaining[offset..];
        let token = after
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '/'))
            .next()
            .unwrap_or_default();
        let mut parts = token.split('/');
        if let (Some(owner), Some(repo)) = (parts.next(), parts.next()) {
            refs.push(format!("{owner}/{repo}"));
        }
        remaining = &after[token.len()..];
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::public_artifact_boundary_blockers;

    #[test]
    fn public_artifact_boundary_rejects_plural_private_and_credentials() {
        let blockers = public_artifact_boundary_blockers(
            "release/evidence/final/public-launch-state.json",
            br#"{"repositories_public":["santhsecurity/vyre"],"public_repository":"santhsecurity/Santh","path":"/media/mukund-thiru/SanthData/Santh/private.json","command":"gh repo edit Santh --visibility public","env":"VYRE_RELEASE_REPOS=santhsecurity/vyre","provenance":"token=abc"}"#,
        );
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("repositories_public")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("private Santh path")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("non-Vyre public repository")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("credential-looking")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("visibility mutation")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("VYRE_RELEASE_REPOS")));
    }
}
