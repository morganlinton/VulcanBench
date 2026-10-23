//! Operator settings from `vulcanbench.toml` at the repository root, a port of
//! `harness/settings.py`: the file names effort levels that never run, and the
//! engine refuses them before any model call.

use std::path::{Path, PathBuf};

use crate::error::EngineError;
use crate::json::Value;
use crate::pyfmt::py_repr_str;
use crate::toml;

/// The checkout this engine was built from (the parent of `engine-rs/`).
/// The Python harness locates the same file relative to its own package.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn settings_path() -> PathBuf {
    repo_root().join("vulcanbench.toml")
}

fn load_settings(path: &Path) -> Result<Value, EngineError> {
    if !path.exists() {
        return Ok(Value::object());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| EngineError::io(format!("reading {}", path.display()), e))?;
    toml::parse(&text).map_err(|e| EngineError::Invalid(format!("{}: {e}", path.display())))
}

/// The lowercased `[effort].blocked` list.
///
/// # Errors
///
/// Fails when the file is malformed or the entry is not a list of strings.
pub fn blocked_efforts(path: &Path) -> Result<Vec<String>, EngineError> {
    let settings = load_settings(path)?;
    let values = settings
        .get("effort")
        .and_then(|effort| effort.get("blocked"))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let Value::Array(items) = values else {
        return Err(bad_blocked(path));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(|s| s.trim().to_lowercase())
                .ok_or_else(|| bad_blocked(path))
        })
        .collect()
}

fn bad_blocked(path: &Path) -> EngineError {
    EngineError::Invalid(format!(
        "{}: [effort].blocked must be a list of strings",
        path.display()
    ))
}

/// Refuses a blocked effort level before any run starts.
///
/// # Errors
///
/// `EngineError::Invalid` carrying the Python `BlockedEffortError` message.
pub fn check_effort_allowed(effort: Option<&str>, path: &Path) -> Result<(), EngineError> {
    let Some(effort) = effort else {
        return Ok(());
    };
    if blocked_efforts(path)?.contains(&effort.trim().to_lowercase()) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(EngineError::Invalid(format!(
            "effort {} is blocked by {name} ([effort].blocked); VulcanBench never runs this level",
            py_repr_str(effort)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_blocked_levels_from_the_checked_in_file() {
        let path = settings_path();
        assert!(path.is_file(), "vulcanbench.toml should sit beside engine-rs/");
        assert!(check_effort_allowed(None, &path).is_ok());
        assert!(check_effort_allowed(Some("high"), &path).is_ok());
        let err = check_effort_allowed(Some(" Ultra "), &path).expect_err("ultra is blocked");
        assert_eq!(
            err.to_string(),
            "effort ' Ultra ' is blocked by vulcanbench.toml ([effort].blocked); VulcanBench never runs this level"
        );
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn a_missing_file_means_no_restrictions() {
        let missing = Path::new("/nonexistent/vulcanbench.toml");
        assert!(check_effort_allowed(Some("ultra"), missing).is_ok());
    }
}
