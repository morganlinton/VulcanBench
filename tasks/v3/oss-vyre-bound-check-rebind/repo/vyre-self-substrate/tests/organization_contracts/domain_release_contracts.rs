use super::*;

#[test]
fn quality_gates_live_under_quality_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let legacy_quality_root = source_root.join("quality");
    let quality_root = source_root.join("integration").join("quality");
    assert!(
        !legacy_quality_root.exists(),
        "legacy src/quality/ must stay empty or absent; quality gates belong under src/integration/quality/"
    );

    assert!(
        quality_root.join("mod.rs").exists(),
        "quality gates must be grouped behind src/integration/quality/mod.rs"
    );

    for module in QUALITY_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "quality module {module} must not live at src/ root; move it under src/integration/quality/"
        );

        let quality_path = quality_root.join(module);
        assert!(
            quality_path.exists(),
            "quality module {module} must live under src/integration/quality/"
        );
    }
}

#[test]
fn optimization_contracts_live_under_optimizer_contracts_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let optimization_facade = source_root.join("optimization").join("mod.rs");
    let contracts_root = source_root.join("optimizer").join("contracts");

    assert!(
        optimization_facade.exists(),
        "historic src/optimization/mod.rs facade must remain for compatibility"
    );
    let facade_source = std::fs::read_to_string(&optimization_facade)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", optimization_facade.display()));
    assert!(
        facade_source.contains("pub use crate::optimizer::contracts"),
        "optimization facade must re-export optimizer::contracts instead of owning implementation files"
    );
    assert!(
        contracts_root.join("mod.rs").exists(),
        "optimizer contracts must be grouped behind src/optimizer/contracts/mod.rs"
    );
    let contracts_mod =
        std::fs::read_to_string(contracts_root.join("mod.rs")).unwrap_or_else(|err| {
            panic!(
                "{} must be readable: {err}",
                contracts_root.join("mod.rs").display()
            )
        });

    for module in OPTIMIZER_CONTRACT_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "optimization module {module} must not live at src/ root; move it under src/optimizer/contracts/"
        );

        let old_optimization_path = source_root.join("optimization").join(module);
        assert!(
            !old_optimization_path.exists(),
            "optimization module {module} must not live beside the compatibility facade; move implementation into src/optimizer/contracts/"
        );

        let contracts_path = contracts_root.join(module);
        assert!(
            contracts_path.exists(),
            "optimization module {module} must live under src/optimizer/contracts/"
        );

        let stem = module
            .strip_suffix(".rs")
            .expect("optimizer contract entries must be Rust source files");
        assert!(
            contracts_mod.contains(&format!("mod {stem};")),
            "optimizer/contracts/mod.rs must declare mod {stem}; so contract imports cross one optimizer-owned boundary"
        );
    }
}

#[test]
fn math_kernels_live_under_math_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let math_root = source_root.join("math");

    assert!(
        math_root.join("mod.rs").exists(),
        "advanced math kernels must be grouped behind src/math/mod.rs"
    );

    for module in MATH_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "math module {module} must not live at src/ root; move it under src/math/"
        );

        let math_path = math_root.join(module);
        assert!(
            math_path.exists(),
            "math module {module} must live under src/math/"
        );
    }
}

#[test]
fn coverage_contracts_live_under_coverage_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let legacy_coverage_root = source_root.join("coverage");
    let coverage_root = source_root.join("integration").join("coverage");
    assert!(
        !legacy_coverage_root.exists(),
        "legacy src/coverage/ must stay empty or absent; coverage contracts belong under src/integration/coverage/"
    );

    assert!(
        coverage_root.join("mod.rs").exists(),
        "coverage contracts must be grouped behind src/integration/coverage/mod.rs"
    );

    for module in COVERAGE_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "coverage module {module} must not live at src/ root; move it under src/integration/coverage/"
        );

        let coverage_path = coverage_root.join(module);
        assert!(
            coverage_path.exists(),
            "coverage module {module} must live under src/integration/coverage/"
        );
    }
}

#[test]
fn evidence_validators_live_under_evidence_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let legacy_evidence_root = source_root.join("evidence");
    let evidence_root = source_root.join("integration").join("evidence");
    assert!(
        !legacy_evidence_root.exists(),
        "legacy src/evidence/ must stay empty or absent; evidence validators belong under src/integration/evidence/"
    );

    assert!(
        evidence_root.join("mod.rs").exists(),
        "evidence validators must be grouped behind src/integration/evidence/mod.rs"
    );

    for module in EVIDENCE_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "evidence module {module} must not live at src/ root; move it under src/integration/evidence/"
        );

        let evidence_path = evidence_root.join(module);
        assert!(
            evidence_path.exists(),
            "evidence module {module} must live under src/integration/evidence/"
        );
    }
}

#[test]
fn hardware_contracts_live_under_hardware_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let hardware_root = source_root.join("hardware");

    assert!(
        hardware_root.join("mod.rs").exists(),
        "hardware contracts must be grouped behind src/hardware/mod.rs"
    );

    for module in HARDWARE_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "hardware module {module} must not live at src/ root; move it under src/hardware/"
        );

        let hardware_path = hardware_root.join(module);
        assert!(
            hardware_path.exists(),
            "hardware module {module} must live under src/hardware/"
        );
    }
}

#[test]
fn release_gates_live_under_release_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let legacy_release_root = source_root.join("release");
    let release_root = source_root.join("integration").join("release");
    assert!(
        !legacy_release_root.exists(),
        "legacy src/release/ must stay empty or absent; release gates belong under src/integration/release/"
    );

    assert!(
        release_root.join("mod.rs").exists(),
        "release gates must be grouped behind src/integration/release/mod.rs"
    );

    for gate in RELEASE_GATES {
        let root_path = source_root.join(gate);
        assert!(
            !root_path.exists(),
            "release gate {gate} must not live at src/ root; move it under src/integration/release/"
        );

        let release_path = release_root.join(gate);
        assert!(
            release_path.exists(),
            "release gate {gate} must live under src/integration/release/"
        );
    }
}
