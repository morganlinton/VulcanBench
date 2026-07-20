//! Cross-crate import-path migration contract for promotion patches.

use std::fs;
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vyre-core must live under the Vyre workspace root")
}

fn read_doc(rel: &str) -> String {
    let path = workspace_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("Fix: import-path migration test could not read {}: {error}", path.display())
    })
}

#[test]
fn import_path_migration_test_contract_names_all_promotion_gates() {
    let crate_graph = read_doc("docs/CRATE_GRAPH.md");
    let primitives_tier = read_doc("docs/primitives-tier.md");
    let library_tiers = read_doc("docs/library-tiers.md");

    for text in [&crate_graph, &primitives_tier, &library_tiers] {
        assert!(text.contains("Cross-crate promotion patch contract"));
        assert!(text.contains("import-path migration test"));
        assert!(text.contains("check-tier-deps"));
        assert!(text.contains("lego-audit"));
    }
}
