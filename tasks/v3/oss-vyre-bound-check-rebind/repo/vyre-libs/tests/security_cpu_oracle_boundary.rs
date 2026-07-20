//! Security CPU oracles must not leak into the public production API.

use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_CPU_API_MARKERS: &[&str] = &[
    "pub fn cpu_",
    "pub fn cpu_ref",
    "pub fn cpu_ref_one_step",
    "pub fn cpu_dominator_sets",
];

#[test]
fn security_modules_do_not_export_cpu_oracle_apis() {
    let security_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("security");
    let mut files = Vec::new();
    collect_rust_files(&security_dir, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("security source file must be readable");
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if PUBLIC_CPU_API_MARKERS
                .iter()
                .any(|marker| trimmed.starts_with(marker))
            {
                violations.push(format!("{}:{}: {trimmed}", path.display(), idx + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "security CPU oracles are parity-only and must stay crate-private; \
         public APIs must build GPU Programs.\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("security source directory must be readable") {
        let entry = entry.expect("security source entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
