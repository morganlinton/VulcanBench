//! Public API snapshot gate contract (`scripts/check_public_api.sh`).

use std::process::Command;

use super::workspace_root;

#[test]
fn public_api_snapshots_match_live_surface() {
    let workspace = workspace_root();
    let script = workspace.join("scripts/check_public_api.sh");
    assert!(
        script.is_file(),
        "public API gate script must exist: {}",
        script.display()
    );

    let output = Command::new("bash")
        .arg(&script)
        .current_dir(&workspace)
        .output()
        .expect("check_public_api.sh should execute");

    assert!(
        output.status.success(),
        "public API surface must match committed PUBLIC_API.md snapshots.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
