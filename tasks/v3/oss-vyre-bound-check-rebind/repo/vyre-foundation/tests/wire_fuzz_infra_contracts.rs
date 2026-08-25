//! Wire-format fuzz infrastructure contracts.
//!
//! `Program::from_wire` is an untrusted parser surface. These tests keep the
//! libFuzzer target, corpus layout, and nightly release hook from drifting.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Fix: vyre-foundation must live under the workspace root.")
        .to_path_buf()
}

#[test]
fn program_wire_fuzz_target_is_registered_and_checks_parser_invariants() {
    let root = workspace_root();
    let cargo = fs::read_to_string(root.join("vyre-foundation/fuzz/Cargo.toml"))
        .expect("Fix: vyre-foundation fuzz Cargo.toml must be readable.");
    let target = fs::read_to_string(root.join("vyre-foundation/fuzz/fuzz_targets/program_wire.rs"))
        .expect("Fix: program_wire fuzz target must be readable.");
    let registry_target =
        fs::read_to_string(root.join("vyre-foundation/fuzz/fuzz_targets/registry_toml.rs"))
            .expect("Fix: registry_toml fuzz target must be readable.");
    let readme = fs::read_to_string(root.join("vyre-foundation/fuzz/README.md"))
        .expect("Fix: fuzz README must be readable.");
    let workflow = fs::read_to_string(root.join(".github/workflows/fuzz.yml"))
        .expect("Fix: fuzz workflow must be readable.");

    for required in [
        "cargo-fuzz = true",
        "libfuzzer-sys",
        "[[bin]]",
        "name = \"program_wire\"",
        "path = \"fuzz_targets/program_wire.rs\"",
        "test = false",
        "doc = false",
        "bench = false",
    ] {
        assert!(
            cargo.contains(required),
            "Fix: fuzz Cargo.toml must keep `{required}` for the wire parser target."
        );
    }

    for required in [
        "Program::from_wire(data)",
        "msg.contains(\"Fix:\")",
        ".to_wire()",
        "Program::from_wire(&round)",
        "program.structural_eq(&reparsed)",
    ] {
        assert!(
            target.contains(required),
            "Fix: program_wire fuzz target must assert `{required}`."
        );
    }

    assert!(
        readme.contains("cargo fuzz run program_wire"),
        "Fix: fuzz README must document how to run the wire parser fuzz target."
    );
    assert!(
        cargo.contains("name = \"registry_toml\"")
            && cargo.contains("path = \"fuzz_targets/registry_toml.rs\""),
        "Fix: fuzz Cargo.toml must keep the registry_toml parser target registered."
    );
    assert!(
        registry_target.contains("data.len() > 64 * 1024")
            && registry_target.contains("std::str::from_utf8(data)")
            && registry_target.contains("toml::from_str::<toml::Value>(s)"),
        "Fix: registry_toml fuzz target must bound input, require UTF-8, and exercise TOML decoding."
    );
    assert!(
        readme.contains("cargo fuzz run registry_toml"),
        "Fix: fuzz README must document how to run the registry TOML fuzz target."
    );
    for target_name in ["decoder", "program_wire", "registry_toml"] {
        assert!(
            workflow.contains(&format!("- {target_name}")),
            "Fix: fuzz workflow matrix must run registered fuzz target `{target_name}` in PR smoke and scheduled CI."
        );
    }
}

#[test]
fn program_wire_fuzz_corpus_is_nontrivial_and_contains_named_valid_programs() {
    let corpus_dir = workspace_root().join("vyre-foundation/fuzz/corpus/program_wire");
    let entries = fs::read_dir(&corpus_dir)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", corpus_dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("fuzz corpus entry must be readable: {error}"))
                .path()
        })
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

    assert!(
        entries.len() >= 64,
        "Fix: program_wire fuzz corpus has only {} entries; keep regression seeds checked in.",
        entries.len()
    );
    for named_seed in [
        "empty_program.vir0",
        "literal_u32_store.vir0",
        "bin_op_add.vir0",
        "if_then_else.vir0",
        "barrier_only.vir0",
    ] {
        assert!(
            corpus_dir.join(named_seed).is_file(),
            "Fix: program_wire fuzz corpus must keep named valid-program seed `{named_seed}`."
        );
    }
}

#[test]
fn nightly_ci_runs_wire_parser_fuzz_infrastructure_contract() {
    let nightly = fs::read_to_string(workspace_root().join("scripts/nightly_ci.sh"))
        .expect("Fix: nightly_ci.sh must be readable.");
    let required =
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-foundation --test wire_fuzz_infra_contracts";
    assert!(
        nightly.contains(required),
        "Fix: nightly CI must run the wire fuzz infrastructure contract: `{required}`."
    );
}
