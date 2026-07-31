#![allow(missing_docs)]
use std::fs;
use std::path::PathBuf;

pub(crate) fn write_scaffold_file(path: PathBuf, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Fix: create directory {} for scaffold output: {error}",
                parent.display()
            )
        })?;
    }
    if path.exists() {
        return Err(format!(
            "Fix: scaffold path {} already exists; delete it first before regenerating",
            path.display()
        ));
    }
    fs::write(&path, contents)
        .map_err(|error| format!("Fix: write scaffold file {}: {error}", path.display()))?;
    Ok(())
}
