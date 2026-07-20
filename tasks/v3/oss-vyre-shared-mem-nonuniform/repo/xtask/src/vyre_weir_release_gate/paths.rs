use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::types::MAX_RELEASE_GATE_TEXT_BYTES;

pub(super) fn manifest_path_from_args(args: &[String]) -> Result<PathBuf, String> {
    let mut manifest_path = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --manifest requires a path.".to_string());
                };
                manifest_path = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- vyre-release-gate [--manifest PATH]\n\n\
                     Checks the Vyre release evidence manifest and fails until every \
                     requirement is closed with concrete evidence files."
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!("Fix: unknown vyre-release-gate option `{other}`."));
            }
        }
    }

    Ok(manifest_path.unwrap_or_else(default_manifest_path))
}
pub(super) fn default_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/vyre-release-evidence.toml"))
        .unwrap_or_else(|| PathBuf::from("release/vyre-release-evidence.toml"))
}
pub(super) fn resolve_manifest_path(base_dir: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    }
}
pub(super) fn resolve_artifact_path(base_dir: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return candidate;
    }
    if path.starts_with("release/") {
        return base_dir
            .parent()
            .map(|workspace| workspace.join(candidate))
            .unwrap_or_else(|| base_dir.join(path));
    }
    base_dir.join(candidate)
}
pub(super) fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_RELEASE_GATE_TEXT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_RELEASE_GATE_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_RELEASE_GATE_TEXT_BYTES} byte release gate read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
