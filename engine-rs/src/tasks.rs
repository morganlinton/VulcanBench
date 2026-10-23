//! Task loading, the task hash and workspace preparation (`harness/tasks.py`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::EngineError;
use crate::json::{self, Value};
use crate::pyfmt::{py_repr_str, universal_newlines};
use crate::sha256::Sha256;

const LOCAL_JUNK_DIRS: [&str; 4] = ["__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache"];
const LOCAL_JUNK_FILES: [&str; 1] = [".DS_Store"];
const LOCAL_JUNK_SUFFIXES: [&str; 2] = ["pyc", "pyo"];

/// A task definition on disk.
#[derive(Debug, Clone)]
pub struct Task {
    pub task_id: String,
    pub root: PathBuf,
    pub metadata: Value,
    pub issue: String,
    pub verifier: Option<PathBuf>,
}

impl Task {
    pub fn snapshot(&self) -> Option<PathBuf> {
        let snap = self.root.join("repo_snapshot.tar.gz");
        snap.exists().then_some(snap)
    }

    pub fn repo_dir(&self) -> Option<PathBuf> {
        let dir = self.root.join("repo");
        dir.is_dir().then_some(dir)
    }

    pub fn hidden_tests_dir(&self) -> Option<PathBuf> {
        let dir = self.root.join("tests");
        dir.is_dir().then_some(dir)
    }

    /// Declarative `fail_to_pass` / `pass_to_pass` spec, when it is an object.
    pub fn tests_spec(&self) -> Option<&Value> {
        self.metadata
            .get("tests")
            .filter(|spec| matches!(spec, Value::Object(_)))
    }

    pub fn gold_patch(&self) -> Option<PathBuf> {
        let path = self.root.join("gold_patch.diff");
        path.exists().then_some(path)
    }

    /// `metadata.setup` as `(name, cmd)` pairs; empty unless every entry has both.
    pub fn setup_commands(&self) -> Vec<(String, String)> {
        let Some(Value::Array(entries)) = self.metadata.get("setup") else {
            return Vec::new();
        };
        let pairs: Option<Vec<(String, String)>> = entries
            .iter()
            .map(|entry| {
                let name = entry.get("name")?;
                let cmd = entry.get("cmd")?;
                Some((crate::pyfmt::py_str(name), crate::pyfmt::py_str(cmd)))
            })
            .collect();
        pairs.unwrap_or_default()
    }

    pub fn setup_timeout_s(&self) -> u64 {
        match self.metadata.get("setup_timeout_s") {
            Some(Value::Int(i)) if *i > 0 => u64::try_from(*i).unwrap_or(600),
            Some(Value::Float(f)) if *f > 0.0 => *f as u64,
            _ => 600,
        }
    }
}

/// Whether `rel` is a local byproduct that is never part of a task.
pub fn is_local_junk(rel: &Path) -> bool {
    let in_junk_dir = rel
        .components()
        .any(|c| LOCAL_JUNK_DIRS.contains(&c.as_os_str().to_string_lossy().as_ref()));
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let suffix = rel
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    in_junk_dir
        || LOCAL_JUNK_FILES.contains(&name.as_str())
        || LOCAL_JUNK_SUFFIXES.contains(&suffix.as_str())
}

/// Loads a task definition from `tasks_root / task_id`.
///
/// # Errors
///
/// `Invalid` with Python's `FileNotFoundError` text when the directory is missing.
pub fn load_task(task_id: &str, tasks_root: &Path) -> Result<Task, EngineError> {
    let root = tasks_root.join(task_id);
    if !root.is_dir() {
        return Err(EngineError::Invalid(format!(
            "task {} not found under {}",
            py_repr_str(task_id),
            tasks_root.display()
        )));
    }
    let meta_path = root.join("metadata.json");
    let metadata = if meta_path.exists() {
        let text = read_text(&meta_path)?;
        json::parse(&text)
            .map_err(|e| EngineError::Invalid(format!("{}: {e}", meta_path.display())))?
    } else {
        Value::object()
    };
    let issue_path = root.join("issue.md");
    let issue = if issue_path.exists() {
        read_text(&issue_path)?
    } else {
        String::new()
    };
    let verifier = root.join("verifier.py");
    Ok(Task {
        task_id: task_id.to_owned(),
        metadata,
        issue,
        verifier: verifier.exists().then_some(verifier),
        root,
    })
}

/// Sorted directory names under `tasks_root`.
pub fn list_task_ids(tasks_root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(tasks_root) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

/// `Path.read_text(encoding="utf-8")`: UTF-8 with universal newlines.
pub fn read_text(path: &Path) -> Result<String, EngineError> {
    let bytes = fs::read(path).map_err(|e| EngineError::io(format!("reading {}", path.display()), e))?;
    let text = String::from_utf8(bytes).map_err(|e| {
        EngineError::Invalid(format!("{} is not valid UTF-8: {e}", path.display()))
    })?;
    Ok(universal_newlines(&text))
}

/// Every regular file below `root`, as paths relative to it. Symlinked
/// directories are not descended, matching `Path.rglob("*")`.
pub fn walk_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_path_buf());
                }
            }
        }
    }
    Ok(out)
}

fn path_parts(path: &Path) -> Vec<String> {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}

/// Files under `root` in `sorted(Path)` order, which compares path components,
/// not the joined string (`a/b` sorts before `a.txt`).
fn sorted_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = walk_files(root)?
        .into_iter()
        .filter(|rel| !is_local_junk(rel))
        .collect();
    files.sort_by_cached_key(|rel| path_parts(rel));
    Ok(files)
}

fn hash_dir(hasher: &mut Sha256, label: &str, root: &Path) -> Result<(), EngineError> {
    let files = sorted_files(root).map_err(|e| EngineError::io(format!("walking {}", root.display()), e))?;
    for rel in files {
        let bytes = fs::read(root.join(&rel))
            .map_err(|e| EngineError::io(format!("reading {}", root.join(&rel).display()), e))?;
        hasher.update(label.as_bytes());
        hasher.update(b"\0");
        hasher.update(path_parts(&rel).join("/").as_bytes());
        hasher.update(b"\0");
        hasher.update(&bytes);
        hasher.update(b"\0");
    }
    Ok(())
}

/// `json.dumps(value, sort_keys=True)`: every object sorted by key, recursively.
fn sorted_keys(value: &Value) -> Value {
    match value {
        Value::Object(entries) => {
            let mut sorted: Vec<(String, Value)> = entries
                .iter()
                .map(|(k, v)| (k.clone(), sorted_keys(v)))
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted_keys).collect()),
        other => other.clone(),
    }
}

/// A deterministic SHA-256 of a task's scoring-relevant definition.
///
/// # Errors
///
/// I/O failures while reading the task tree.
pub fn task_hash(task: &Task) -> Result<String, EngineError> {
    let mut hasher = Sha256::new();
    if let Some(repo) = task.repo_dir() {
        hash_dir(&mut hasher, "repo", &repo)?;
    } else if let Some(snapshot) = task.snapshot() {
        hasher.update(b"snapshot\0");
        hasher.update(&fs::read(snapshot).map_err(|e| EngineError::io("reading snapshot", e))?);
        hasher.update(b"\0");
    }
    hasher.update(b"issue\0");
    hasher.update(task.issue.as_bytes());
    if let Some(compose) = task
        .metadata
        .get("environment")
        .and_then(|env| env.get("compose"))
        .and_then(Value::as_str)
    {
        let compose_path = task.root.join(compose);
        if compose_path.is_file() {
            hasher.update(b"environment\0");
            hasher.update(&fs::read(compose_path).map_err(|e| EngineError::io("reading compose", e))?);
            hasher.update(b"\0");
        }
    }
    if let Some(tests) = task.hidden_tests_dir() {
        hash_dir(&mut hasher, "tests", &tests)?;
    }
    hasher.update(b"\0tests_spec\0");
    let spec = task.tests_spec().cloned().unwrap_or_else(Value::object);
    hasher.update(sorted_keys(&spec).to_json().as_bytes());
    hasher.update(b"\0verifier\0");
    if let Some(verifier) = &task.verifier {
        hasher.update(&fs::read(verifier).map_err(|e| EngineError::io("reading verifier", e))?);
    }
    hasher.update(b"\0gold\0");
    if let Some(gold) = task.gold_patch() {
        hasher.update(&fs::read(gold).map_err(|e| EngineError::io("reading gold patch", e))?);
    }
    Ok(hasher.hexdigest())
}

/// Paths (relative to the workspace) excluded from list/search by `.vulcanbenchignore`.
pub fn vulcanbenchignore_paths(workspace: &Path) -> Vec<PathBuf> {
    let ignore_file = workspace.join(".vulcanbenchignore");
    let Ok(text) = fs::read_to_string(&ignore_file) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(PathBuf::from)
        .collect()
}

pub fn should_ignore(rel: &Path, ignored: &[PathBuf]) -> bool {
    let rel_posix = path_parts(rel).join("/");
    ignored.iter().any(|pattern| {
        let pattern_posix = path_parts(pattern).join("/");
        if rel == pattern || rel_posix.starts_with(&format!("{pattern_posix}/")) {
            return true;
        }
        // A bare name (no directory part) also matches as the first component.
        let bare = pattern.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true);
        bare && rel.components().next().is_some_and(|first| first.as_os_str() == pattern.as_os_str())
    })
}

fn copytree(src: &Path, dst: &Path) -> Result<(), EngineError> {
    fs::create_dir_all(dst).map_err(|e| EngineError::io(format!("creating {}", dst.display()), e))?;
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| EngineError::io(format!("listing {}", dir.display()), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| EngineError::io("listing task files", e))?;
            let path = entry.path();
            let rel = path.strip_prefix(src).unwrap_or(&path).to_path_buf();
            if is_local_junk(&rel) {
                continue;
            }
            let target = dst.join(&rel);
            if path.is_dir() {
                fs::create_dir_all(&target)
                    .map_err(|e| EngineError::io(format!("creating {}", target.display()), e))?;
                stack.push(path);
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| EngineError::io(format!("creating {}", parent.display()), e))?;
                }
                fs::copy(&path, &target)
                    .map_err(|e| EngineError::io(format!("copying {}", path.display()), e))?;
            }
        }
    }
    Ok(())
}

/// Materializes the task's starting state into `workspace` (hidden tests excluded).
///
/// # Errors
///
/// I/O failures, or a snapshot the system `tar` cannot extract.
pub fn prepare_workspace(task: &Task, workspace: &Path) -> Result<(), EngineError> {
    fs::create_dir_all(workspace)
        .map_err(|e| EngineError::io(format!("creating {}", workspace.display()), e))?;
    if let Some(repo) = task.repo_dir() {
        copytree(&repo, workspace)?;
    } else if let Some(snapshot) = task.snapshot() {
        // The Python engine extracts with tarfile and refuses members that
        // escape the destination; the spike leans on the host tar, which
        // refuses absolute and parent-relative members by default.
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(&snapshot)
            .arg("-C")
            .arg(workspace)
            .status()
            .map_err(|e| EngineError::io("running tar", e))?;
        if !status.success() {
            return Err(EngineError::Invalid(format!(
                "extracting {} failed with {status}",
                snapshot.display()
            )));
        }
    }
    fs::write(workspace.join("issue.md"), &task.issue)
        .map_err(|e| EngineError::io("writing issue.md", e))?;
    let ignore_src = task.root.join(".vulcanbenchignore");
    if ignore_src.is_file() {
        fs::copy(&ignore_src, workspace.join(".vulcanbenchignore"))
            .map_err(|e| EngineError::io("copying .vulcanbenchignore", e))?;
    }
    Ok(())
}

/// Overlays the hidden `tests/` tree onto the workspace at verification time.
///
/// # Errors
///
/// I/O failures while copying.
pub fn install_hidden_tests(task: &Task, workspace: &Path) -> Result<(), EngineError> {
    match task.hidden_tests_dir() {
        Some(tests) => copytree(&tests, workspace),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        crate::settings::repo_root()
    }

    #[test]
    fn hello_world_hash_matches_the_python_engine() {
        let task = load_task("hello-world", &repo_root().join("tasks/v1")).expect("task loads");
        assert_eq!(
            task_hash(&task).expect("hashes"),
            "489facdbea89c2f7c82a59abbeb5a4a373bfdf56c988bcb7596b67ca38f14c7f"
        );
        assert!(task.verifier.is_some());
        assert!(task.tests_spec().is_none());
        assert!(task.setup_commands().is_empty());
    }

    #[test]
    fn junk_and_ignore_rules() {
        assert!(is_local_junk(Path::new("a/__pycache__/x.py")));
        assert!(is_local_junk(Path::new("x.pyc")));
        assert!(is_local_junk(Path::new("dir/.DS_Store")));
        assert!(!is_local_junk(Path::new("src/main.py")));
        let ignored = vec![PathBuf::from("vendor"), PathBuf::from("docs/gen")];
        assert!(should_ignore(Path::new("vendor/x.py"), &ignored));
        assert!(should_ignore(Path::new("docs/gen/a.md"), &ignored));
        assert!(!should_ignore(Path::new("docs/a.md"), &ignored));
        assert!(!should_ignore(Path::new("src/vendor/x.py"), &ignored));
    }

    #[test]
    fn sorted_keys_is_recursive() {
        let value = json::parse(r#"{"b": {"z": 1, "a": 2}, "a": [ {"y": 1, "x": 2} ]}"#).expect("parses");
        assert_eq!(
            sorted_keys(&value).to_json(),
            r#"{"a": [{"x": 2, "y": 1}], "b": {"a": 2, "z": 1}}"#
        );
    }
}
