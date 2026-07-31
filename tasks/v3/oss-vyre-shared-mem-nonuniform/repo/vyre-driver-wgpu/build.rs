//! Export the resolved Naga version to the crate so disk cache keys
//! invalidate cleanly when the shader frontend changes. Reads from
//! this crate's own Cargo.toml under [package.metadata.vyre], because
//! the workspace root is not available inside the crates.io tarball.

use std::fs;
use std::path::PathBuf;

const MAX_BUILD_MANIFEST_BYTES: u64 = 1024 * 1024;

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("Fix: {message}");
    std::process::exit(1);
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| {
        fail("CARGO_MANIFEST_DIR missing; restore this invariant before continuing.")
    }));
    let manifest_path = manifest_dir.join("Cargo.toml");
    let manifest = read_manifest_bounded(&manifest_path).unwrap_or_else(|error| {
        fail(format!(
            "failed to read {}: {error}",
            manifest_path.display()
        ))
    });
    let naga_version = manifest
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            let key = "naga_version = \"";
            if !trimmed.starts_with(key) {
                return None;
            }
            let rest = &trimmed[key.len()..];
            let end = rest.find('"')?;
            Some(rest[..end].to_string())
        })
        .unwrap_or_else(|| {
            fail(format!(
                "failed to locate `naga_version = \"...\"` under [package.metadata.vyre] in {}",
                manifest_path.display()
            ))
        });

    println!("cargo:rerun-if-changed={}", manifest_path.display());
    println!("cargo:rustc-env=VYRE_NAGA_VERSION={naga_version}");
}

fn read_manifest_bounded(path: &std::path::Path) -> Result<String, String> {
    let mut reader = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 4096];
    loop {
        let read =
            std::io::Read::read(&mut reader, &mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return String::from_utf8(bytes).map_err(|error| error.to_string());
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > MAX_BUILD_MANIFEST_BYTES {
            return Err(format!(
                "manifest exceeds {MAX_BUILD_MANIFEST_BYTES} byte build-script cap"
            ));
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}
