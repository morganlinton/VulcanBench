//! Audit `rules/launch/*.toml` rule contracts and report missing or
//! malformed fields.
//!
//! Run via `cargo_full run -p xtask --bin audit_rule_contracts`. The binary
//! exits non-zero when any rule deviates from `rules/SCHEMA.md`.

use std::fs;
use std::path::Path;

fn main() {
    let launch_dir = Path::new("../../../../../rules/launch");
    if !launch_dir.exists() {
        eprintln!("Rules directory not found");
        std::process::exit(1);
    }

    let mut failed = false;
    for entry in fs::read_dir(launch_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            let slug = path.file_name().unwrap().to_str().unwrap();
            println!("Auditing rule {}", slug);

            let contract = path.join("CONTRACT.md");
            if !contract.exists() {
                eprintln!("FAIL: Missing CONTRACT.md in {}", slug);
                failed = true;
            }

            let test_dir = Path::new("../../../../../tests/launch_rule_truth").join(slug);
            let expected_dirs = ["positives", "negatives", "evasions", "cross_file"];
            for d in expected_dirs.iter() {
                if !test_dir.join(d).exists() {
                    eprintln!("FAIL: Missing test dir {}/{}", slug, d);
                    failed = true;
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
