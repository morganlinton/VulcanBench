//! Tests for source/header slice minimization.

use vyre_frontend_c::api::{ParityMinimizerConfig, ParitySourceFile, ParitySourceMinimizer};

#[test]
fn minimizer_removes_unneeded_files_and_lines_while_preserving_mismatch() {
    let files = vec![
        ParitySourceFile::new(
            "linux/lib/math/main.c",
            concat!(
                "#include \"noise.h\"\n",
                "int before;\n",
                "int trigger = 1;\n",
                "int after;\n",
            ),
        ),
        ParitySourceFile::new(
            "linux/lib/math/noise.h",
            concat!("int noise_a;\n", "int noise_b;\n"),
        ),
    ];
    let minimizer = ParitySourceMinimizer::new(ParityMinimizerConfig {
        min_lines_per_file: 1,
        max_predicate_evaluations: 128,
    });

    let reduced = minimizer.minimize(files, |candidate| {
        candidate
            .iter()
            .any(|file| file.text.contains("trigger = 1"))
    });

    assert_eq!(reduced.files.len(), 1);
    assert_eq!(reduced.files[0].path, "linux/lib/math/main.c");
    assert_eq!(reduced.files[0].text, "int trigger = 1;\n");
    assert!(reduced.predicate_evaluations > 1);
}

#[test]
fn minimizer_preserves_configured_minimum_line_count() {
    let files = vec![ParitySourceFile::new(
        "linux/lib/math/main.c",
        concat!("int keep_a;\n", "int trigger = 1;\n", "int keep_b;\n"),
    )];
    let minimizer = ParitySourceMinimizer::new(ParityMinimizerConfig {
        min_lines_per_file: 2,
        max_predicate_evaluations: 128,
    });

    let reduced = minimizer.minimize(files, |candidate| candidate[0].text.contains("trigger = 1"));

    assert_eq!(reduced.files[0].line_count(), 2);
    assert!(reduced.files[0].text.contains("trigger = 1"));
}

#[test]
fn minimizer_keeps_original_when_initial_input_does_not_reproduce() {
    let files = vec![ParitySourceFile::new("linux/lib/math/main.c", "int x;\n")];

    let reduced = ParitySourceMinimizer::default().minimize(files.clone(), |candidate| {
        candidate[0].text.contains("missing_trigger")
    });

    assert_eq!(reduced.files, files);
    assert_eq!(reduced.predicate_evaluations, 1);
}
