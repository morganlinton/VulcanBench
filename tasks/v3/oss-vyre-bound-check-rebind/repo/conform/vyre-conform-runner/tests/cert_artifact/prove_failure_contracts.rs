use super::*;

#[test]
fn prove_refuses_certificate_when_backend_cannot_dispatch() {
    // In the default build only SPIR-V (emission-only, no device) and
    // photonic (non-dispatching hardware substrate) are linked. Neither can execute a program, so
    // `prove` MUST refuse to emit the certificate.
    let out_path = std::env::temp_dir().join(format!(
        "vyre-conform-prove-refuses-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&out_path);
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "vyre-conform-runner",
            "--no-default-features",
            "--quiet",
            "--",
            "prove",
            "--out",
        ])
        .arg(&out_path)
        .output()
        .expect("Fix: cargo must be available in PATH");
    assert!(
        !output.status.success(),
        "TEST-034: prove without a dispatch-capable backend must exit non-zero; stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refused to emit"),
        "TEST-034: prove must explain why it refused to emit the certificate; stderr={stderr}"
    );
    assert!(
        !out_path.exists(),
        "TEST-034: prove must not leave a certificate file on disk when parity fails"
    );
}

// Drives `cargo run -p vyre-conform-runner --features gpu -- prove`; GPU

