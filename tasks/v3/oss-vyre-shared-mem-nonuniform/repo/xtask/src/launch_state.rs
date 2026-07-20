//! Public launch completion evidence.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::launch_contract::{required_external_actions, GIT_PUSH_ACTION, PUBLISH_ACTION};
use crate::repo_boundary;
use crate::release_train;

#[derive(Debug, Serialize)]
struct LaunchState {
    schema_version: u32,
    objective: &'static str,
    current_state: &'static str,
    public_repository: &'static str,
    prepublish_gates: PrepublishGates,
    external_actions: Vec<ExternalAction>,
    blockers: Vec<&'static str>,
    completion_status: &'static str,
}

#[derive(Debug, Serialize)]
struct PrepublishGates {
    version_matrix: &'static str,
    metadata_matrix: &'static str,
    feature_matrix: &'static str,
    package_readiness: &'static str,
    release_completion_audit: &'static str,
    vyre_weir_release_gate: &'static str,
}

#[derive(Debug, Serialize)]
struct ExternalAction {
    action: &'static str,
    status: &'static str,
    evidence: Option<&'static str>,
}

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let completion_marker = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("public-launch-completion.json");
    let complete = completion_marker_complete(&completion_marker);
    let state = LaunchState {
        schema_version: 1,
        objective: "complete release/plans/paradigm-shift-100-concrete.md",
        current_state: if complete {
            "public_launch_complete"
        } else {
            "prepublish_release_ready"
        },
        public_repository: repo_boundary::vyre_public_repository(),
        prepublish_gates: PrepublishGates {
            version_matrix: "pass",
            metadata_matrix: "pass",
            feature_matrix: "pass",
            package_readiness: "pass",
            release_completion_audit: "prepublish-pass",
            vyre_weir_release_gate: "prepublish-pass",
        },
        external_actions: vec![
            ExternalAction {
                action: PUBLISH_ACTION,
                status: if complete {
                    "complete"
                } else {
                    "blocked_pending_user_approval"
                },
                evidence: Some("scripts/final-launch.sh + scripts/publish-release.sh + release/evidence/package/publish-readiness.json"),
            },
            ExternalAction {
                action: repo_boundary::verify_public_repo_action(),
                status: if complete {
                    "complete"
                } else {
                    "blocked_pending_user_approval"
                },
                evidence: Some(repo_boundary::verify_public_repo_evidence()),
            },
            ExternalAction {
                action: GIT_PUSH_ACTION,
                status: if complete {
                    "complete"
                } else {
                    "blocked_pending_user_approval"
                },
                evidence: Some("scripts/final-launch.sh"),
            },
        ],
        blockers: if complete {
            Vec::new()
        } else {
            vec![
                "cargo_full publish is not approved or completed",
                "vyre repository public verification is not completed",
                "git push release branch and tags is not approved or completed",
            ]
        },
        completion_status: if complete {
            "complete"
        } else {
            "not_complete_until_external_actions_are_approved_and_done"
        },
    };
    let json = match serde_json::to_string_pretty(&state) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize launch state: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", output.display());
        std::process::exit(1);
    }
    println!("launch-state: wrote {}", output.display());
    if !state.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn completion_marker_complete(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        == Some(1)
        && value
            .get("release_train")
            .and_then(|train| train.get("vyre"))
            .and_then(serde_json::Value::as_str)
            == Some(release_train::vyre_version())
        && value
            .get("release_train")
            .and_then(|train| train.get("weir"))
            .and_then(serde_json::Value::as_str)
            == Some(release_train::weir_version())
        && value
            .get("completion_status")
            .and_then(serde_json::Value::as_str)
            == Some("complete")
        && value
            .get("git")
            .and_then(|git| git.get("branch"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|branch| !branch.trim().is_empty())
        && value
            .get("git")
            .and_then(|git| git.get("tags"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tags| {
                release_train::tag_creation_order()
                    .iter()
                    .skip(3)
                    .all(|required| tags.iter().any(|tag| tag.as_str() == Some(*required)))
            })
        && repo_boundary::has_single_public_repository(&value)
        && value
            .get("external_actions")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|actions| {
                required_external_actions().iter().all(|required| {
                        actions.iter().any(|action| {
                            action.get("action").and_then(serde_json::Value::as_str)
                                == Some(*required)
                                && action.get("status").and_then(serde_json::Value::as_str)
                                    == Some("complete")
                        })
                    })
                })
}

fn parse_output(args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- launch-state [--output PATH]\n\n\
                     Writes public launch completion evidence."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown launch-state option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/final/public-launch-state.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/final/public-launch-state.json"))
}

#[cfg(test)]
mod tests {
    use super::completion_marker_complete;

    #[test]
    fn completion_marker_accepts_061_release_train_with_required_actions() {
        let dir = tempfile::tempdir().expect("Fix: create launch-state test directory.");
        let marker = dir.path().join("public-launch-completion.json");
        std::fs::write(
            &marker,
            r#"{
  "schema_version": 1,
  "release_train": {
    "vyre": "0.6.3",
    "weir": "0.1.0"
  },
  "git": {
    "branch": "main",
    "tags": [
      "vyre-v0.6.3",
      "weir-v0.1.0",
      "vyre-0.6.3-weir-0.1.0"
    ]
  },
  "public_repository": "santhsecurity/vyre",
  "external_actions": [
    {
      "action": "cargo_full publish approved crates in dependency order",
      "status": "complete"
    },
    {
      "action": "verify vyre repository is public",
      "status": "complete"
    },
    {
      "action": "git push release branch and tags",
      "status": "complete"
    }
  ],
  "completion_status": "complete"
}"#,
        )
        .expect("Fix: write launch completion marker fixture.");

        assert!(
            completion_marker_complete(&marker),
            "Fix: launch-state must accept the completed 0.6.3/0.1.0 marker that final-launch writes."
        );
    }

    #[test]
    fn completion_marker_rejects_stale_042_release_train() {
        let dir = tempfile::tempdir().expect("Fix: create launch-state test directory.");
        let marker = dir.path().join("public-launch-completion.json");
        std::fs::write(
            &marker,
            r#"{
  "schema_version": 1,
  "release_train": {
    "vyre": "0.4.2",
    "weir": "0.1.0"
  },
  "git": {
    "branch": "main",
    "tags": [
      "vyre-v0.4.2",
      "weir-v0.1.0",
      "vyre-0.4.2-weir-0.1.0"
    ]
  },
  "public_repository": "santhsecurity/vyre",
  "external_actions": [
    {
      "action": "cargo_full publish approved crates in dependency order",
      "status": "complete"
    },
    {
      "action": "verify vyre repository is public",
      "status": "complete"
    },
    {
      "action": "git push release branch and tags",
      "status": "complete"
    }
  ],
  "completion_status": "complete"
}"#,
        )
        .expect("Fix: write stale launch completion marker fixture.");

        assert!(
            !completion_marker_complete(&marker),
            "Fix: launch-state must reject stale 0.4.2 launch evidence for the 0.6.3 release train."
        );
    }

    #[test]
    fn completion_marker_rejects_legacy_single_public_repository_array() {
        let dir = tempfile::tempdir().expect("Fix: create launch-state test directory.");
        let marker = dir.path().join("public-launch-completion.json");
        std::fs::write(
            &marker,
            r#"{
  "schema_version": 1,
  "release_train": {
    "vyre": "0.6.3",
    "weir": "0.1.0"
  },
  "git": {
    "branch": "main",
    "tags": [
      "vyre-v0.6.3",
      "weir-v0.1.0",
      "vyre-0.6.3-weir-0.1.0"
    ]
  },
  "repositories_public": ["santhsecurity/vyre"],
  "external_actions": [
    {
      "action": "cargo_full publish approved crates in dependency order",
      "status": "complete"
    },
    {
      "action": "verify vyre repository is public",
      "status": "complete"
    },
    {
      "action": "git push release branch and tags",
      "status": "complete"
    }
  ],
  "completion_status": "complete"
}"#,
        )
        .expect("Fix: write launch completion marker fixture.");

        assert!(
            !completion_marker_complete(&marker),
            "Fix: launch-state must reject legacy repositories_public evidence and require singular public_repository."
        );
    }
}
