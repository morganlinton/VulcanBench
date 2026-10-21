//! SPIR-V emitter contracts without depending on another concrete driver.

use std::io::Write;
use std::process::Command;

use tempfile::NamedTempFile;
use vyre_driver_spirv::SpirvBackend;

fn minimal_compute_module() -> naga::Module {
    naga::front::wgsl::parse_str(
        r#"
@group(0) @binding(0)
var<storage, read_write> out: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    out[gid.x] = gid.x + 1u;
}
"#,
    )
    .expect("Fix: minimal SPIR-V emitter fixture must parse as WGSL")
}

fn assert_spirv_structural_invariants(label: &str, words: &[u32]) {
    assert!(
        !words.is_empty(),
        "Fix: {label} emitted an empty SPIR-V blob"
    );
    assert_eq!(
        words[0], 0x0723_0203,
        "Fix: {label} emitted a SPIR-V blob without the SPIR-V magic header"
    );

    if Command::new("spirv-val").arg("--version").output().is_ok() {
        let mut file = NamedTempFile::new()
            .unwrap_or_else(|error| panic!("Fix: create temp SPIR-V file for {label}: {error}"));
        for word in words {
            file.write_all(&word.to_le_bytes())
                .unwrap_or_else(|error| panic!("Fix: write SPIR-V bytes for {label}: {error}"));
        }
        let output = Command::new("spirv-val")
            .arg(file.path())
            .output()
            .unwrap_or_else(|error| panic!("Fix: launch spirv-val for {label}: {error}"));
        assert!(
            output.status.success(),
            "Fix: spirv-val rejected {label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert!(
            words.len() >= 5,
            "Fix: {label} emitted a truncated SPIR-V header fallback snapshot"
        );
        assert!(
            words[1] >= 0x0001_0000,
            "Fix: {label} emitted an invalid SPIR-V version word in fallback validation"
        );
    }
}

#[test]
fn valid_naga_module_emits_valid_spirv() {
    let module = minimal_compute_module();
    let words = SpirvBackend::emit_spv(&module)
        .unwrap_or_else(|error| panic!("Fix: emit minimal compute SPIR-V: {error}"));
    assert_spirv_structural_invariants("minimal_compute_module", &words);
}

#[test]
fn emission_is_deterministic_for_identical_modules() {
    let first = SpirvBackend::emit_spv(&minimal_compute_module())
        .expect("Fix: first minimal compute SPIR-V emission must succeed");
    let second = SpirvBackend::emit_spv(&minimal_compute_module())
        .expect("Fix: second minimal compute SPIR-V emission must succeed");
    assert_eq!(
        first, second,
        "Fix: SPIR-V emission must be deterministic for identical modules"
    );
}
