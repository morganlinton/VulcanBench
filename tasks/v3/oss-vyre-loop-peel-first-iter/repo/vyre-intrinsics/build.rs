//! Build-time category-classification gate (F-IR-34).
//!
//! Scans `OpDefRegistration` source blocks across the workspace and fails
//! the build if any op claims `Category::Composite` (Category A) while also carrying a
//! `primary_text: Some(...)` lowering arm.  That shape is the classification
//! drift this gate exists to prevent: a pure-IR composition that secretly
//! requires a dedicated Naga emitter arm breaks on any backend that lacks
//! that arm.
//!
//! The scan is heuristic (string-based) because it must run before the crate
//! is compiled and cannot depend on `inventory::iter` at build time.  Every
//! `OpDefRegistration` block in the tree follows a consistent layout, so the
//! heuristic is reliable for the current source style.

use std::fs;
use std::io::Read;
use std::path::Path;

const MAX_CATEGORY_SCAN_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("Fix: {message}");
    std::process::exit(1);
}

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent().unwrap_or_else(|| {
        fail("vyre-intrinsics must live under the vyre workspace root; restore this invariant before continuing.")
    });

    let files = [
        workspace.join("vyre-driver/src/registry/core_indirect.rs"),
        workspace.join("vyre-driver/src/registry/io.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_add.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_and.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_compare_exchange.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_exchange.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_lru_update.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_max.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_min.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_or.rs"),
        workspace.join("vyre-libs/src/math/atomic/atomic_xor.rs"),
    ];

    for path in &files {
        if !path.exists() {
            continue;
        }
        let src = read_category_scan_source_bounded(path).unwrap_or_else(|e| {
            fail(format!(
                "cannot read {} for category scan: {e}",
                path.display()
            ))
        });
        check_file(path, &src);
    }
}

fn read_category_scan_source_bounded(path: &Path) -> Result<String, String> {
    let mut reader = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > MAX_CATEGORY_SCAN_SOURCE_BYTES {
            return Err(format!(
                "source file exceeds {MAX_CATEGORY_SCAN_SOURCE_BYTES} byte category-scan cap"
            ));
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn check_file(path: &Path, src: &str) {
    let mut cursor = 0;
    while let Some(off) = src[cursor..].find("OpDefRegistration::new") {
        let block_start = cursor + off;
        // Locate the inner `OpDef { ... }` block.
        let Some(opdef_off) = src[block_start..].find("OpDef {") else {
            cursor = block_start + 1;
            continue;
        };
        let opdef_start = block_start + opdef_off;
        let Some(opdef_len) = find_matching_brace(&src[opdef_start..]) else {
            cursor = opdef_start + 1;
            continue;
        };
        let block = &src[opdef_start..opdef_start + opdef_len];

        let is_composite = block.contains("category: Category::Composite");
        let has_naga_some = block.contains("primary_text: Some(");

        if is_composite && has_naga_some {
            fail(format!(
                "category classification mismatch for op in `{}`: declared Composite (Category A) but lowering table has primary_text: Some(...). Fix: Category A ops must be pure IR composition with no dedicated Naga arm.",
                path.display()
            ));
        }

        cursor = opdef_start + opdef_len;
    }
}

fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}
