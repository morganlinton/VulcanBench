//! Decode modules must expose GPU builders and explicit reference oracles, not CPU aliases.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn decode_modules_do_not_export_public_cpu_aliases() {
    let decode_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("decode");
    let mut files = Vec::new();
    collect_rust_files(&decode_dir, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("decode source file must be readable");
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub fn ") && trimmed.contains("_cpu") {
                violations.push(format!("{}:{}: {trimmed}", path.display(), idx + 1));
            }
            if trimmed.starts_with("pub use ") && trimmed.contains("_cpu") {
                violations.push(format!("{}:{}: {trimmed}", path.display(), idx + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "decode CPU aliases are not public production APIs; expose `*_gpu` \
         builders or explicit `*_reference` oracles instead.\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("decode source directory must be readable") {
        let entry = entry.expect("decode source entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
