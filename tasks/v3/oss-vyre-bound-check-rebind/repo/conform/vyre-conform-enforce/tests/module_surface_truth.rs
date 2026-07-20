//! Source-backed gates for module surfaces called out by the half-migration ledger.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
struct Surface {
    mod_file: &'static str,
    min_children: usize,
}

const SURFACES: &[Surface] = &[
    Surface {
        mod_file: "vyre-primitives/src/visual/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-primitives/src/nn/mod.rs",
        min_children: 2,
    },
    Surface {
        mod_file: "vyre-primitives/src/vfs/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-primitives/src/opt/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-primitives/src/types/mod.rs",
        min_children: 2,
    },
    Surface {
        mod_file: "vyre-primitives/src/decode/mod.rs",
        min_children: 3,
    },
    Surface {
        mod_file: "vyre-primitives/src/label/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-primitives/src/nfa/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-primitives/src/topology/mod.rs",
        min_children: 3,
    },
    Surface {
        mod_file: "vyre-primitives/src/cat/mod.rs",
        min_children: 3,
    },
    Surface {
        mod_file: "vyre-primitives/src/fixpoint/mod.rs",
        min_children: 2,
    },
    Surface {
        mod_file: "vyre-primitives/src/geom/mod.rs",
        min_children: 2,
    },
    Surface {
        mod_file: "vyre-primitives/src/effects/mod.rs",
        min_children: 3,
    },
    Surface {
        mod_file: "vyre-primitives/src/reduce/mod.rs",
        min_children: 16,
    },
    Surface {
        mod_file: "vyre-primitives/src/zx/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-primitives/src/dnnf/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-libs/src/test_support/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-libs/src/matching/substring/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-libs/src/math/broadcast/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-libs/src/math/scan/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-libs/src/parsing/core/mod.rs",
        min_children: 2,
    },
    Surface {
        mod_file: "vyre-libs/src/representation/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-bench/src/evolve/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-foundation/src/optimizer/passes/specialization/mod.rs",
        min_children: 1,
    },
    Surface {
        mod_file: "vyre-foundation/src/optimizer/passes/sync/mod.rs",
        min_children: 1,
    },
];

#[test]
fn half_migration_module_surfaces_have_real_children() {
    let root = workspace_root();
    for surface in SURFACES {
        let mod_path = root.join(surface.mod_file);
        let source = read_to_string(&mod_path);
        assert!(
            source
                .lines()
                .any(|line| line.trim_start().starts_with("//!")),
            "Fix: `{}` must document its module contract, not act as an anonymous shell.",
            surface.mod_file
        );

        let children = declared_children(&source);
        assert!(
            children.len() >= surface.min_children,
            "Fix: `{}` declares {} child module(s), expected at least {} real implementation child(ren).",
            surface.mod_file,
            children.len(),
            surface.min_children
        );

        let module_dir = mod_path
            .parent()
            .unwrap_or_else(|| panic!("Fix: `{}` must have a parent dir.", surface.mod_file));
        for child in children {
            let child_path = resolve_child_module(module_dir, &child).unwrap_or_else(|| {
                panic!(
                    "Fix: `{}` declares child module `{child}`, but `{child}.rs` or `{child}/mod.rs` does not exist.",
                    surface.mod_file
                )
            });
            assert_real_implementation(surface.mod_file, &child, &child_path);
        }
    }
}

fn declared_children(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))?;
            let name = rest
                .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
                .next()
                .unwrap_or_default();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

fn assert_real_implementation(parent: &str, child: &str, path: &Path) {
    let source = read_to_string(path);
    let code_lines = source
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("//")
                && !line.starts_with("#![allow(")
                && !line.starts_with("#[cfg(")
        })
        .count();
    assert!(
        code_lines >= 3,
        "Fix: `{parent}` child module `{child}` at `{}` is too thin to be a real implementation surface.",
        path.display()
    );
}

fn resolve_child_module(module_dir: &Path, child: &str) -> Option<PathBuf> {
    let flat = module_dir.join(format!("{child}.rs"));
    if flat.exists() {
        return Some(flat);
    }
    let nested = module_dir.join(child).join("mod.rs");
    nested.exists().then_some(nested)
}

fn read_to_string(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("Fix: read `{}`: {error}", path.display()))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("Fix: conform crate must live two levels below the workspace root.")
}
