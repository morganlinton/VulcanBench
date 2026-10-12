//! Boundary tests for self-substrate graph wrappers.
//!
//! The graph algorithms are owned by `vyre-primitives`. Self-substrate may add
//! dispatch, scratch, cache, and resident wiring, but it must not fork the
//! primitive algorithm bodies back into its public graph entry modules.

use std::fs;
use std::path::{Path, PathBuf};

const WRAPPERS: &[&str] = &[
    "adaptive_traverse",
    "alias_registry",
    "csr_bidirectional",
    "csr_forward_or_changed",
    "dominator_frontier",
    "exploded",
    "motif",
    "path_reconstruct",
    "persistent_bfs",
    "toposort",
];

#[test]
fn graph_wrappers_remain_thin_primitive_dispatch_layers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = root
        .parent()
        .expect("Fix: vyre-self-substrate must live in the Vyre workspace root.");
    for wrapper in WRAPPERS {
        let primitive = workspace
            .join("vyre-primitives/src/graph")
            .join(format!("{wrapper}.rs"));
        let substrate = module_source_path(&root.join("src/graph"), wrapper);

        let primitive_source = read_source(&primitive);
        let substrate_source = read_source(&substrate);
        let primitive_lines = source_lines(&primitive_source);
        let substrate_lines = source_lines(&substrate_source);

        assert!(
            primitive_lines > substrate_lines,
            "Fix: graph wrapper `{wrapper}` is not thinner than its primitive authority: primitive={primitive_lines} self={substrate_lines}."
        );
        assert!(
            substrate_lines <= 100,
            "Fix: graph wrapper `{wrapper}` has {substrate_lines} lines; move algorithm logic to vyre-primitives and keep only dispatch/scratch wiring here."
        );
        assert!(
            wrapper_mentions_primitive(&root, wrapper, &substrate_source),
            "Fix: graph wrapper `{wrapper}` must delegate through vyre_primitives::graph rather than carrying a private algorithm fork."
        );
    }
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "Fix: graph wrapper boundary test could not read `{}`: {error}",
            path.display()
        )
    })
}

fn module_source_path(root: &Path, module: &str) -> PathBuf {
    let flat = root.join(format!("{module}.rs"));
    if flat.is_file() {
        return flat;
    }
    root.join(module).join("mod.rs")
}

fn source_lines(source: &str) -> usize {
    source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn wrapper_mentions_primitive(root: &Path, wrapper: &str, substrate_source: &str) -> bool {
    if substrate_source.contains("vyre_primitives::graph") {
        return true;
    }
    let wrapper_dir = root.join("src/graph").join(wrapper);
    let Ok(entries) = fs::read_dir(&wrapper_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        if read_source(&path).contains("vyre_primitives::graph") {
            return true;
        }
    }
    false
}
