//! Source provenance: which commit of VulcanBench produced a run, a port of
//! `harness/provenance.py`. Read once per process, like the Python module.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use crate::json::{obj, Value};
use crate::process;

/// Paths whose contents can change what a run is asked to do or how it is scored.
pub const SCORING_PATHS: [&str; 6] = [
    "harness",
    "tasks",
    "sandbox",
    "vulcanbench.toml",
    "pyproject.toml",
    "uv.lock",
];

pub const ALLOW_DIRTY_ENV: &str = "VULCANBENCH_ALLOW_DIRTY";

const MAX_DIRTY_PATHS: usize = 50;

/// One git checkout's commit and its uncommitted scoring-relevant paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutState {
    pub root: String,
    pub commit: Option<String>,
    pub describe: Option<String>,
    pub dirty_paths: Vec<String>,
}

impl CheckoutState {
    pub fn dirty(&self) -> bool {
        !self.dirty_paths.is_empty()
    }

    pub fn as_summary(&self) -> Value {
        obj! {
            "root" => self.root.as_str(),
            "commit" => self.commit.as_deref(),
            "describe" => self.describe.as_deref(),
            "dirty" => self.dirty(),
            "dirty_paths" => self.dirty_paths.clone(),
        }
    }
}

/// Where the engine code and the tasks of a run came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProvenance {
    pub harness: Option<CheckoutState>,
    pub tasks: Option<CheckoutState>,
}

impl SourceProvenance {
    pub fn dirty(&self) -> bool {
        [&self.harness, &self.tasks]
            .into_iter()
            .flatten()
            .any(CheckoutState::dirty)
    }

    pub fn as_summary(&self) -> Value {
        obj! {
            "harness" => self.harness.as_ref().map(CheckoutState::as_summary),
            "tasks" => self.tasks.as_ref().map(CheckoutState::as_summary),
            "dirty" => self.dirty(),
            "allow_dirty" => allow_dirty_from_env(),
        }
    }
}

fn git(args: &[&str], cwd: &Path, strip: bool) -> Option<String> {
    let mut command = Command::new("git");
    command.args(args).current_dir(cwd);
    let done = process::run(&mut command, Some(Duration::from_secs(30))).ok()?;
    if done.timed_out || done.exit_code != 0 {
        return None;
    }
    Some(if strip {
        done.stdout.trim().to_owned()
    } else {
        done.stdout
    })
}

fn checkout_state(path: &Path) -> Option<CheckoutState> {
    let root = git(&["rev-parse", "--show-toplevel"], path, true).filter(|r| !r.is_empty())?;
    let top = Path::new(&root);
    let mut status_args = vec!["status", "--porcelain", "--"];
    status_args.extend(SCORING_PATHS);
    let status = git(&status_args, top, false).unwrap_or_default();
    let dirty_paths: Vec<String> = status
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().skip(3).collect())
        .take(MAX_DIRTY_PATHS)
        .collect();
    Some(CheckoutState {
        commit: git(&["rev-parse", "HEAD"], top, true),
        describe: git(&["describe", "--tags", "--always", "--dirty"], top, true),
        root,
        dirty_paths,
    })
}

fn compute(cwd: &Path) -> SourceProvenance {
    let harness = checkout_state(Path::new(env!("CARGO_MANIFEST_DIR")));
    let here = checkout_state(cwd);
    let tasks = match (&harness, here) {
        (Some(h), Some(t)) if t.root != h.root => Some(t),
        (None, Some(t)) => Some(t),
        _ => None,
    };
    SourceProvenance { harness, tasks }
}

/// Provenance for runs launched from the current working directory.
pub fn source_provenance() -> &'static SourceProvenance {
    static CACHE: OnceLock<SourceProvenance> = OnceLock::new();
    CACHE.get_or_init(|| {
        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        compute(&cwd)
    })
}

pub fn allow_dirty_from_env() -> bool {
    std::env::var(ALLOW_DIRTY_ENV)
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}
