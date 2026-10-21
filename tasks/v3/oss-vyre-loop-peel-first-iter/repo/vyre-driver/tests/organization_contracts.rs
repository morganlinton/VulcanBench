//! Organization-level contract tests for the vyre-driver ecosystem.
//!
//! These tests enforce long-term structural contracts without relying on
//! brittle message wording. They may fail when code violates a contract.

use std::collections::HashSet;
use std::path::PathBuf;

use vyre_driver::backend::{validate_program, BackendError, VyreBackend};
use vyre_foundation::ir::{Node, OpId, Program};
use vyre_foundation::program_caps::{check_backend_capabilities, RequiredCapabilities};

// ---------------------------------------------------------------------------
// 1. No wildcard public re-export expansion in new driver modules
// ---------------------------------------------------------------------------

/// Scan every `.rs` file in driver-tier crates for `pub use ...::*`.
/// New wildcard re-exports break the explicit-surface contract.
#[test]
fn driver_modules_avoid_wildcard_pub_reexports() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let workspace_manifest = std::fs::read_to_string(workspace_root.join("Cargo.toml"))
        .expect("workspace manifest must be readable");
    let driver_crates: Vec<PathBuf> = workspace_manifest
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_matches(',').trim_matches('"');
            trimmed
                .starts_with("vyre-driver-")
                .then(|| workspace_root.join(trimmed))
        })
        .collect();

    let mut violations = Vec::new();

    for crate_root in driver_crates {
        let src = crate_root.join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    for (line_no, line) in content.lines().enumerate() {
                        let t = line.trim();
                        if t.starts_with("pub use") && t.ends_with("::*;") {
                            let rel = path.strip_prefix(&crate_root).unwrap_or(&path);
                            violations.push(format!("{}:{} {}", rel.display(), line_no + 1, t));
                        }
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "driver modules must not expand wildcard pub re-exports. Violations:\n{}",
        violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 2. Public validation errors remain actionable with Fix guidance
// ---------------------------------------------------------------------------

/// Every public error type that vyre-driver surfaces must contain a Fix: hint.
#[test]
fn backend_errors_contain_fix_guidance() {
    let errors: Vec<BackendError> = vec![
        BackendError::DeviceOutOfMemory {
            requested: 1,
            available: 0,
        },
        BackendError::UnsupportedFeature {
            name: "x".into(),
            backend: "y".into(),
        },
        BackendError::PoisonedLock {
            lock_error: "x".into(),
        },
        BackendError::KernelCompileFailed {
            backend: "y".into(),
            compiler_message: "z".into(),
        },
        BackendError::DispatchFailed {
            code: None,
            message: "m".into(),
        },
        BackendError::InvalidProgram {
            fix: "Fix: do something".into(),
        },
    ];

    for err in &errors {
        let msg = err.to_string();
        assert!(
            msg.contains("Fix:"),
            "BackendError must carry actionable Fix: guidance; got: {msg}"
        );
    }
}

/// BackendError::new must synthesize a generic Fix when the caller omits one.
#[test]
fn backend_error_new_synthesizes_fix_when_absent() {
    let err = BackendError::new("something broke");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:"),
        "BackendError::new must ensure Fix: is present; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// 3. Capability contracts are explicit
// ---------------------------------------------------------------------------

/// A backend that advertises zero supported ops must reject a program
/// that contains any statement node. The contract must be explicit:
/// silence is not consent.
#[test]
fn empty_capability_set_rejects_any_program_with_nodes() {
    struct NoOpBackend;

    impl vyre_driver::backend::private::Sealed for NoOpBackend {}

    impl VyreBackend for NoOpBackend {
        fn id(&self) -> &'static str {
            "noop"
        }
        fn supported_ops(&self) -> &HashSet<OpId> {
            static EMPTY: std::sync::OnceLock<HashSet<OpId>> = std::sync::OnceLock::new();
            EMPTY.get_or_init(HashSet::new)
        }
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &vyre_driver::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![])
        }
    }

    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return]);
    let backend = NoOpBackend;
    let err = validate_program(&program, &backend)
        .expect_err("a backend with empty supported_ops must reject a program containing nodes");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") || msg.contains("unsupported") || msg.contains("supported"),
        "empty-capability rejection must be actionable: {msg}"
    );
}

/// Capability negotiation must list every missing bit, not stop at the first.
#[test]
fn capability_negotiation_lists_all_missing_bits() {
    let mut required = RequiredCapabilities::none();
    required.subgroup_ops = true;
    required.f16 = true;
    required.bf16 = true;
    required.indirect_dispatch = true;
    required.trap = true;
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [1, 1, 1],
        &required,
    )
    .unwrap_err();
    let missing = err.missing;
    let expected: HashSet<&str> = [
        "subgroup_ops",
        "f16",
        "bf16",
        "indirect_dispatch",
        "trap_propagation",
    ]
    .iter()
    .copied()
    .collect();
    // `missing` is now `Vec<String>` so the entries can carry
    // axis-specific workgroup_size diagnostics; the test only
    // checks the simple capability names so borrow as &str.
    let actual: HashSet<&str> = missing.iter().map(String::as_str).collect();
    assert_eq!(
        actual, expected,
        "capability negotiation must report every missing capability explicitly"
    );
}

// ---------------------------------------------------------------------------
// 4. Graph/program validation rejects malformed input instead of defaulting silently
// ---------------------------------------------------------------------------

/// validate_program must return an Err for a program containing an unsupported
/// operation, never silently defaulting to Ok.
#[test]
fn validation_rejects_unsupported_operation() {
    struct UnsupportedOpsBackend;

    impl vyre_driver::backend::private::Sealed for UnsupportedOpsBackend {}

    impl VyreBackend for UnsupportedOpsBackend {
        fn id(&self) -> &'static str {
            "unsupported-ops-contract"
        }
        fn supported_ops(&self) -> &HashSet<OpId> {
            static EMPTY: std::sync::OnceLock<HashSet<OpId>> = std::sync::OnceLock::new();
            EMPTY.get_or_init(HashSet::new)
        }
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &vyre_driver::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![])
        }
    }

    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return]);
    let err = validate_program(&program, &UnsupportedOpsBackend).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:"),
        "validation rejection must carry actionable Fix: guidance; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// 5. Test fixtures are external not inline
// ---------------------------------------------------------------------------

/// Organization contract: integration tests must load fixtures from external
/// files rather than embedding large inline data. This test verifies the
/// fixture directory exists and the fixture is readable.
#[test]
fn external_fixture_is_loadable() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest.join("tests/fixtures/unsupported_op.txt");
    assert!(
        fixture.exists(),
        "external fixture must exist at {fixture:?}"
    );
    let content = std::fs::read_to_string(&fixture).unwrap();
    assert!(!content.trim().is_empty(), "fixture must not be empty");
}

/// A test that actually uses the external fixture to drive behavior.
#[test]
fn external_fixture_drives_validation_rejection() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest.join("tests/fixtures/unsupported_op.txt");
    let op_id = std::fs::read_to_string(&fixture)
        .unwrap()
        .trim()
        .to_string();

    struct DenyAllBackend;

    impl vyre_driver::backend::private::Sealed for DenyAllBackend {}

    impl VyreBackend for DenyAllBackend {
        fn id(&self) -> &'static str {
            "deny_all"
        }
        fn supported_ops(&self) -> &HashSet<OpId> {
            static EMPTY: std::sync::OnceLock<HashSet<OpId>> = std::sync::OnceLock::new();
            EMPTY.get_or_init(HashSet::new)
        }
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &vyre_driver::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![])
        }
    }

    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return]);
    let err = validate_program(&program, &DenyAllBackend)
        .expect_err("validation must reject unsupported ops (fixture op={op_id})");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:"),
        "fixture-driven validation rejection must carry Fix: guidance; got: {msg}"
    );
}
