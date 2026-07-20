use super::*;

#[test]
fn telemetry_lives_under_telemetry_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let telemetry_root = source_root.join("telemetry");

    assert!(
        telemetry_root.join("mod.rs").exists(),
        "telemetry must be grouped behind src/telemetry/mod.rs"
    );

    for module in TELEMETRY_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "telemetry module {module} must not live at src/ root; move it under src/telemetry/"
        );

        let telemetry_path = telemetry_root.join(module);
        assert!(
            telemetry_path.exists(),
            "telemetry module {module} must live under src/telemetry/"
        );
    }
}

#[test]
fn source_root_only_contains_crate_entrypoint() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let mut root_files = std::fs::read_dir(&source_root)
        .expect("src directory must be readable")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    root_files.sort();

    assert_eq!(
        root_files,
        vec!["lib.rs".to_string()],
        "self-substrate source root must stay a namespace table, not a flat module dump"
    );
}

#[test]
fn data_substrate_lives_under_data_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let data_root = source_root.join("data");

    assert!(
        data_root.join("mod.rs").exists(),
        "data substrate must be grouped behind src/data/mod.rs"
    );

    for module in DATA_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "data module {module} must not live at src/ root; move it under src/data/"
        );

        let data_path = data_root.join(module);
        assert!(
            data_path.exists(),
            "data module {module} must live under src/data/"
        );
    }
}

#[test]
fn logic_rewrite_substrate_lives_under_logic_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let logic_root = source_root.join("logic");

    assert!(
        logic_root.join("mod.rs").exists(),
        "logic and rewrite substrate must be grouped behind src/logic/mod.rs"
    );

    for module in LOGIC_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "logic module {module} must not live at src/ root; move it under src/logic/"
        );

        let logic_path = logic_root.join(module);
        assert!(
            logic_path.exists(),
            "logic module {module} must live under src/logic/"
        );
    }
}

#[test]
fn scheduling_strategies_live_under_scheduling_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let scheduling_root = source_root.join("scheduling");

    assert!(
        scheduling_root.join("mod.rs").exists(),
        "scheduling strategies must be grouped behind src/scheduling/mod.rs"
    );

    for module in SCHEDULING_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "scheduling module {module} must not live at src/ root; move it under src/scheduling/"
        );

        let scheduling_path = scheduling_root.join(module);
        assert!(
            scheduling_path.exists(),
            "scheduling module {module} must live under src/scheduling/"
        );
    }
}

#[test]
fn analysis_substrate_lives_under_analysis_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let analysis_root = source_root.join("analysis");

    assert!(
        analysis_root.join("mod.rs").exists(),
        "analysis substrate must be grouped behind src/analysis/mod.rs"
    );

    for module in ANALYSIS_MODULES {
        let root_path = source_root.join(module);
        assert!(
            !root_path.exists(),
            "analysis module {module} must not live at src/ root; move it under src/analysis/"
        );

        let analysis_path = analysis_root.join(module);
        assert!(
            analysis_path.exists(),
            "analysis module {module} must live under src/analysis/"
        );
    }
}

