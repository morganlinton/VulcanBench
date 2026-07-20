//! Bench corpus duplication contract.
//!
//! Files under `benches/competition/corpora/` may duplicate bytes only through
//! the manifest-declared cumulative-prefix policy. They must not duplicate
//! normal test fixtures by content hash.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_corpus_duplicates_match_manifest_policy() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let checker = workspace_root.join("benches/competition/scripts/check_corpora.py");
    assert!(
        checker.is_file(),
        "bench corpus checker is missing at {}. Fix: restore benches/competition/scripts/check_corpora.py or replace this contract with an equivalent release benchmark corpus verifier.",
        checker.display()
    );
    let output = Command::new("python3")
        .arg(&checker)
        .current_dir(workspace_root)
        .output()
        .expect("python3 must be available to run corpus manifest checker");
    assert!(
        output.status.success(),
        "bench corpus duplicate policy failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bench_corpus_does_not_duplicate_test_fixtures() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let bench_corpus = workspace_root.join("benches/competition/corpora");
    let fixture_dirs = [
        workspace_root.join("vyre-libs/fixtures"),
        workspace_root.join("vyre-libs/tests/fixtures"),
        workspace_root.join("vyre-foundation/tests/fixtures"),
        workspace_root.join("vyre-driver/tests/fixtures"),
        workspace_root.join("vyre-primitives/tests/fixtures"),
    ];

    let mut bench_hashes: HashMap<String, Vec<PathBuf>> = HashMap::new();
    if bench_corpus.is_dir() {
        collect_hashes(&bench_corpus, &mut bench_hashes);
    }

    let mut fixture_hashes: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for dir in &fixture_dirs {
        if dir.is_dir() {
            collect_hashes(dir, &mut fixture_hashes);
        }
    }

    let mut duplicates = Vec::new();
    for (hash, bench_paths) in &bench_hashes {
        if let Some(fix_paths) = fixture_hashes.get(hash) {
            for bp in bench_paths {
                for fp in fix_paths {
                    duplicates.push(format!(
                        "{} duplicates {}",
                        bp.strip_prefix(workspace_root).unwrap().display(),
                        fp.strip_prefix(workspace_root).unwrap().display()
                    ));
                }
            }
        }
    }

    assert!(
        duplicates.is_empty(),
        "bench corpus must not duplicate test fixtures by content. Duplicates:\n{}",
        duplicates.join("\n")
    );
}

fn collect_hashes(dir: &std::path::Path, map: &mut HashMap<String, Vec<PathBuf>>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(content) = std::fs::read(&path) {
                let hash = blake3::hash(&content).to_hex().to_string();
                map.entry(hash).or_default().push(path);
            }
        }
    }
}
