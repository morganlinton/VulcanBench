//! Workspace-wide wildcard public re-export baseline.
//!
//! Wildcard `pub use` hubs expand API surface unpredictably. Existing hubs
//! across all workspace crates are baselined; new ones require explicit review.

use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn workspace_wildcard_pub_reexports_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let members = [
        "vyre-core",
        "vyre-foundation",
        "vyre-driver",
        "vyre-reference",
        "vyre-spec",
        "vyre-macros",
        "vyre-primitives",
        "vyre-runtime",
        "vyre-libs",
        "vyre-intrinsics",
        "vyre-frontend-c",
        "vyre-harness",
        "conform/vyre-conform-spec",
        "conform/vyre-conform-generate",
        "conform/vyre-conform-enforce",
        "conform/vyre-conform-runner",
        "conform/vyre-test-harness",
    ];

    // Known existing wildcard pub re-exports. Do not expand without review.
    //
    // ROADMAP HM3: vyre-core's `lower` shim re-exports the canonical
    // `vyre-lower` crate so external consumers can keep importing
    // through `vyre_core::lower::*`. The wildcard is the entire
    // contract of the shim  -  narrowing it would defeat the facade.
    let known: HashSet<String> = [
        "vyre-core/src/lib.rs pub use vyre_lower::*;",
        "vyre-core/src/lib.rs:112 pub use vyre_lower::*;",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations = Vec::new();

    for member in &members {
        let member_root = workspace_root.join(member);
        // Scan both src/ and tests/ for wildcard pub use.
        for sub in ["src", "tests"] {
            let scan_dir = member_root.join(sub);
            if !scan_dir.is_dir() {
                continue;
            }
            let mut stack = vec![scan_dir];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                        let content = std::fs::read_to_string(&path).unwrap();
                        for (line_no, line) in content.lines().enumerate() {
                            let t = line.trim();
                            if t.starts_with("pub use") && t.ends_with("::*;") {
                                let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                                let key = format!("{}:{} {}", rel.display(), line_no + 1, t);
                                if !known.contains(&key) {
                                    let key_no_line = format!("{} {}", rel.display(), t);
                                    if !known.contains(&key_no_line) {
                                        new_violations.push(key);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        new_violations.is_empty(),
        "new wildcard pub re-exports are forbidden. Baseline them only after explicit review. Violations:\n{}",
        new_violations.join("\n")
    );
}
