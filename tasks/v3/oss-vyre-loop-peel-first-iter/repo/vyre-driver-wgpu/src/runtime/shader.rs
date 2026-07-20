//! Shader pipeline compilation and caching.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

/// Compute shader compilation.
pub mod compile_compute_pipeline;

/// Dump WGSL source when `VYRE_DUMP_WGSL` is set.
///
/// The environment variable may contain an output directory. Values `1`, `true`,
/// and `yes` use a private process-local directory under `${TMPDIR}`. Dumps
/// are content-addressed by BLAKE3 so concurrent compiles cannot clobber each
/// other.
pub(crate) fn dump_wgsl_if_requested(label: &str, wgsl_source: &str) -> std::io::Result<()> {
    let Some(target) = std::env::var_os("VYRE_DUMP_WGSL") else {
        return Ok(());
    };
    let (dir, private_dir) = dump_dir(target);
    std::fs::create_dir_all(&dir)?;
    if private_dir {
        make_private_dir(&dir)?;
    }

    let hash = blake3::hash(wgsl_source.as_bytes());
    let label = sanitize_label(label);
    let path = dir.join(format!("{label}-{hash}.wgsl"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error),
    };
    file.write_all(wgsl_source.as_bytes())?;
    file.sync_all()
}

fn dump_dir(target: OsString) -> (PathBuf, bool) {
    let raw = target.to_string_lossy();
    if matches!(raw.as_ref(), "1" | "true" | "yes") {
        return (
            std::env::temp_dir().join(format!("vyre-wgsl-dumps-{}", std::process::id())),
            true,
        );
    }
    (PathBuf::from(target), false)
}

#[cfg(unix)]
fn make_private_dir(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn make_private_dir(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn sanitize_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len().clamp(8, 64));
    for ch in label.chars().take(64) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('-');
        }
    }
    if out.is_empty() {
        out.push_str("shader");
    }
    trim_path_separators(&out).to_owned()
}

fn trim_path_separators(label: &str) -> &str {
    let trimmed = label.trim_matches(|ch| ch == '/' || ch == '\\');
    if trimmed.is_empty() {
        "shader"
    } else {
        trimmed
    }
}
