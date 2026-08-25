//! Scaffold a new vyre Tier-B rule TOML at `rules/<category>/<name>.toml`.
//!
//! Run via `cargo_full run -p xtask --bin scaffold_rule -- <category> <name>`.
//! The binary writes a starter file that already conforms to
//! `rules/SCHEMA.md` so the rule loader accepts it on first parse.

use std::fs;
use std::path::Path;

fn fatal(message: &str) -> ! {
    eprintln!("Fix: {message}");
    std::process::exit(1);
}

fn create_dir(path: &Path) {
    if let Err(error) = fs::create_dir_all(path) {
        eprintln!("Fix: failed to create `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn write_file(path: &Path, contents: &str) {
    if let Err(error) = fs::write(path, contents) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let slug = match args.next() {
        Some(slug) if !slug.trim().is_empty() => slug,
        _ => fatal("expected rule slug; pass one launch-rule slug"),
    };

    let launch_dir = Path::new("../../../../../rules/launch").join(&slug);
    create_dir(&launch_dir);

    write_file(&launch_dir.join("CONTRACT.md"), "# Rule Contract\n");

    let test_dir = Path::new("../../../../../tests/launch_rule_truth").join(&slug);
    create_dir(&test_dir);

    for d in &["positives", "negatives", "evasions", "cross_file"] {
        create_dir(&test_dir.join(d));
    }

    write_file(&test_dir.join("cve_replay.toml"), "");
    write_file(&test_dir.join("property.rs"), "");
    write_file(&test_dir.join("differential.toml"), "");
    write_file(&test_dir.join("e2e_cli.rs"), "");

    println!("Scaffolded rule {}", slug);
}
