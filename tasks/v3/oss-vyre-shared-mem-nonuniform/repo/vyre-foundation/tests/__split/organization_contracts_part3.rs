use super::*;

#[test]
fn agent_skills_artifacts_stay_out_of_production_dirs() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let crate_roots = [
        workspace_root.join("vyre-foundation"),
        workspace_root.join("vyre-libs"),
        workspace_root.join("vyre-primitives"),
        workspace_root.join("vyre-runtime"),
        workspace_root.join("vyre-core"),
        workspace_root.join("vyre-spec"),
        workspace_root.join("vyre-frontend-c"),
        workspace_root.join("vyre-driver"),
        workspace_root.join("vyre-harness"),
        workspace_root.join("vyre-intrinsics"),
        workspace_root.join("vyre-macros"),
        workspace_root.join("vyre-reference"),
        workspace_root.join("conform/vyre-conform-runner"),
        workspace_root.join("conform/vyre-conform-spec"),
        workspace_root.join("conform/vyre-conform-generate"),
        workspace_root.join("conform/vyre-conform-enforce"),
    ];

    let mut found = HashSet::new();

    for crate_root in &crate_roots {
        if !crate_root.is_dir() {
            continue;
        }
        let cargo_toml = crate_root.join("Cargo.toml");
        if !cargo_toml.exists() {
            continue;
        }

        // Scan src/ directory for AGENTS.md / SKILL.md
        let src_dir = crate_root.join("src");
        if src_dir.is_dir() {
            let mut stack = vec![src_dir];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else {
                        let fname = path.file_name().and_then(|s| s.to_str());
                        if fname == Some("AGENTS.md") || fname == Some("SKILL.md") {
                            let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                            found.insert(rel.display().to_string());
                        }
                    }
                }
            }
        }

        // Check crate root for AGENTS.md / SKILL.md
        for name in ["AGENTS.md", "SKILL.md"] {
            let path = crate_root.join(name);
            if path.exists() {
                let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                found.insert(rel.display().to_string());
            }
        }

        // Check for .kimi/ anywhere in crate, excluding tests/benches/examples/target/.internals
        let mut kstack = vec![crate_root.clone()];
        while let Some(dir) = kstack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if path.file_name().and_then(|s| s.to_str()) == Some(".kimi") {
                    let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                    found.insert(rel.display().to_string());
                } else {
                    let fname = path.file_name().unwrap().to_string_lossy();
                    if fname != "target"
                        && fname != "tests"
                        && fname != "benches"
                        && fname != "examples"
                        && fname != ".internals"
                        && !fname.starts_with('.')
                    {
                        kstack.push(path);
                    }
                }
            }
        }
    }

    let known: HashSet<String> = [
        "vyre-libs/src/matching/SKILL.md",
        "vyre-libs/src/math/SKILL.md",
        "vyre-libs/src/nn/SKILL.md",
        "vyre-libs/src/scan/SKILL.md",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations: Vec<String> =
        found.into_iter().filter(|v| !known.contains(v)).collect();
    new_violations.sort();

    assert!(
        new_violations.is_empty(),
        "agent/skills artifacts (AGENTS.md, SKILL.md, .kimi/) are forbidden in production crate dirs. \
         New violations:\n{}",
        new_violations.join("\n")
    );
}
