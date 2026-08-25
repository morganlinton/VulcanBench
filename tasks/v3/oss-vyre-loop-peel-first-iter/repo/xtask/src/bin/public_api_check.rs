//! Run `cargo_full public-api` against facade crates and diff the result
//! against each facade's committed `PUBLIC_API.md`.
//!
//! Run via `cargo_full run -p xtask --bin public_api_check`. The binary
//! exits non-zero when any facade's public-API surface drifts from its
//! frozen snapshot, which is the publish-floor invariant.

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

const MAX_PUBLIC_API_CHECK_TEXT_BYTES: u64 = 4_194_304;

const FACADE_CRATES: &[&str] = &["vyre", "vyre-foundation", "vyre-libs"];

fn main() {
    let mut args = std::env::args().skip(1);
    let is_update = args.next().as_deref() == Some("--update");

    let mut failed = false;

    let root = match workspace_root() {
        Some(root) => root,
        None => {
            eprintln!(
                "Fix: public_api_check must run from an xtask crate with a workspace parent."
            );
            std::process::exit(1);
        }
    };
    let cargo_runner = std::env::var("VYRE_CARGO_RUNNER").unwrap_or_else(|_| "cargo_full".into());

    for crate_name in FACADE_CRATES {
        let output = match Command::new(&cargo_runner)
            .arg("public-api")
            .arg("-p")
            .arg(crate_name)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!(
                    "Fix: failed to execute `{cargo_runner} public-api -p {crate_name}`: {error}"
                );
                failed = true;
                continue;
            }
        };

        if !output.status.success() {
            eprintln!(
                "Failed to generate public API for {}: {}",
                crate_name,
                String::from_utf8_lossy(&output.stderr)
            );
            failed = true;
            continue;
        }

        let new_api = match String::from_utf8(output.stdout) {
            Ok(api) => api,
            Err(error) => {
                eprintln!("Fix: public API output for {crate_name} was not UTF-8: {error}");
                failed = true;
                continue;
            }
        };

        let md_path = match find_crate_dir(crate_name, &root) {
            Ok(Some(p)) => p.join("PUBLIC_API.md"),
            Ok(None) => {
                eprintln!("Could not find dir for crate {}", crate_name);
                failed = true;
                continue;
            }
            Err(error) => {
                eprintln!("Fix: failed while locating crate {crate_name}: {error}");
                failed = true;
                continue;
            }
        };

        if is_update {
            if let Err(error) = fs::write(&md_path, new_api) {
                eprintln!("Fix: failed to write `{}`: {error}", md_path.display());
                failed = true;
                continue;
            }
            println!("Updated {}", md_path.display());
        } else {
            let old_api = match read_text_bounded(&md_path) {
                Ok(api) => api,
                Err(error) => {
                    eprintln!(
                        "Fix: failed to read public API snapshot `{}`: {error}",
                        md_path.display()
                    );
                    failed = true;
                    continue;
                }
            };
            if new_api != old_api {
                eprintln!("Public API drifted for crate {}. Fix: run `cargo_full run --bin xtask -- public-api-update` to regenerate.", crate_name);
                failed = true;
            } else {
                println!("{} API matches snapshot.", crate_name);
            }
        }
    }

    if let Err(errors) = validate_vyre_libs_alias_metadata(&root) {
        for error in errors {
            eprintln!("{error}");
        }
        failed = true;
    }

    if failed && !is_update {
        std::process::exit(1);
    }
}

fn workspace_root() -> Option<std::path::PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}

fn validate_vyre_libs_alias_metadata(root: &Path) -> Result<(), Vec<String>> {
    let registry_path = root.join("vyre-libs/src/compat_aliases.rs");
    let api_path = root.join("vyre-libs/PUBLIC_API.md");
    let registry = read_text_bounded(&registry_path).map_err(|error| {
        vec![format!(
            "Fix: failed to read alias registry `{}`: {error}",
            registry_path.display()
        )]
    })?;
    let api = read_text_bounded(&api_path).map_err(|error| {
        vec![format!(
            "Fix: failed to read vyre-libs public API snapshot `{}`: {error}",
            api_path.display()
        )]
    })?;
    let failures = alias_metadata_failures(&registry, &api);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn alias_metadata_failures(registry: &str, public_api: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for required in [
        "pub struct CompatibilityAlias",
        "deprecated_path",
        "canonical_path",
        "canonical_owner",
        "removal_condition",
        "COMPATIBILITY_ALIASES",
    ] {
        if !registry.contains(required) {
            failures.push(format!(
                "Fix: vyre-libs alias registry is missing `{required}`."
            ));
        }
    }
    for (alias, deprecated_path, canonical_path) in [
        ("MATCHING_ALIAS", "vyre_libs::matching", "vyre_libs::scan"),
        (
            "MATCHING_SUBSTRING_ALIAS",
            "vyre_libs::matching::substring",
            "vyre_libs::scan::substring",
        ),
    ] {
        if !registry.contains(alias) {
            failures.push(format!(
                "Fix: vyre-libs alias registry is missing `{alias}`."
            ));
        }
        if !registry.contains(deprecated_path) {
            failures.push(format!(
                "Fix: alias `{alias}` must name deprecated path `{deprecated_path}`."
            ));
        }
        if !registry.contains(canonical_path) {
            failures.push(format!(
                "Fix: alias `{alias}` must name canonical path `{canonical_path}`."
            ));
        }
    }
    for required_api in [
        "pub mod vyre_libs::compat_aliases",
        "pub mod vyre_libs::matching",
        "pub mod vyre_libs::scan",
        "pub struct vyre_libs::compat_aliases::CompatibilityAlias",
        "pub const vyre_libs::compat_aliases::COMPATIBILITY_ALIASES",
    ] {
        if !public_api.contains(required_api) {
            failures.push(format!(
                "Fix: vyre-libs PUBLIC_API.md is missing `{required_api}`; regenerate the snapshot after alias metadata changes."
            ));
        }
    }
    failures
}

fn find_crate_dir(name: &str, root: &Path) -> Result<Option<std::path::PathBuf>, String> {
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().components().any(|c| c.as_os_str() == "target") {
            continue;
        }
        if entry.file_name() == "Cargo.toml" {
            let content = read_text_bounded(entry.path())
                .map_err(|error| format!("{}: {error}", entry.path().display()))?;
            if content.contains(&format!("name = \"{}\"", name)) {
                return Ok(entry.path().parent().map(Path::to_path_buf));
            }
        }
    }
    Ok(None)
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_PUBLIC_API_CHECK_TEXT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_PUBLIC_API_CHECK_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_PUBLIC_API_CHECK_TEXT_BYTES} byte public API check read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_metadata_accepts_registry_and_snapshot() {
        let registry = r#"
pub struct CompatibilityAlias {
    pub deprecated_path: &'static str,
    pub canonical_path: &'static str,
    pub canonical_owner: &'static str,
    pub removal_condition: &'static str,
}
pub const MATCHING_ALIAS: CompatibilityAlias = CompatibilityAlias {
    deprecated_path: "vyre_libs::matching",
    canonical_path: "vyre_libs::scan",
    canonical_owner: "vyre-libs/src/scan",
    removal_condition: "snapshot no longer requires it",
};
pub const MATCHING_SUBSTRING_ALIAS: CompatibilityAlias = CompatibilityAlias {
    deprecated_path: "vyre_libs::matching::substring",
    canonical_path: "vyre_libs::scan::substring",
    canonical_owner: "vyre-libs/src/scan/substring",
    removal_condition: "snapshot no longer requires it",
};
pub const COMPATIBILITY_ALIASES: &[CompatibilityAlias] = &[MATCHING_ALIAS, MATCHING_SUBSTRING_ALIAS];
"#;
        let public_api = r#"
pub mod vyre_libs::compat_aliases
pub mod vyre_libs::matching
pub mod vyre_libs::scan
pub struct vyre_libs::compat_aliases::CompatibilityAlias
pub const vyre_libs::compat_aliases::COMPATIBILITY_ALIASES
"#;

        assert!(alias_metadata_failures(registry, public_api).is_empty());
    }

    #[test]
    fn alias_metadata_rejects_missing_removal_condition_and_snapshot_alias() {
        let registry = r#"
pub struct CompatibilityAlias {
    pub deprecated_path: &'static str,
    pub canonical_path: &'static str,
    pub canonical_owner: &'static str,
}
pub const MATCHING_ALIAS: CompatibilityAlias = CompatibilityAlias {
    deprecated_path: "vyre_libs::matching",
    canonical_path: "vyre_libs::scan",
    canonical_owner: "vyre-libs/src/scan",
};
pub const COMPATIBILITY_ALIASES: &[CompatibilityAlias] = &[MATCHING_ALIAS];
"#;
        let public_api = r#"
pub mod vyre_libs::compat_aliases
pub mod vyre_libs::scan
pub struct vyre_libs::compat_aliases::CompatibilityAlias
"#;

        let failures = alias_metadata_failures(registry, public_api);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("removal_condition")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("MATCHING_SUBSTRING_ALIAS")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("pub mod vyre_libs::matching")));
    }
}
