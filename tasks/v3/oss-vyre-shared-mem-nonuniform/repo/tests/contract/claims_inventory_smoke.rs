//! Op inventory contract: `cargo xtask list-ops` must enumerate Tier 2.5 primitives.

use std::process::Command;

use super::workspace_root;

#[test]
fn list_ops_inventory_includes_vyre_primitives() {
    let workspace = workspace_root();
    let output = Command::new("cargo")
        .args(["xtask", "list-ops"])
        .current_dir(&workspace)
        .output()
        .expect("cargo xtask list-ops should execute");

    assert!(
        output.status.success(),
        "cargo xtask list-ops must succeed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let inventory = String::from_utf8_lossy(&output.stdout);
    assert!(
        inventory.contains("vyre-primitives::"),
        "list-ops output must include Tier 2.5 vyre-primitives op ids.\nhead:\n{}",
        inventory.lines().take(30).collect::<Vec<_>>().join("\n")
    );
}
