//! Test: source hygiene.
const SOURCE_FILES: &[(&str, &str)] = &[
    (
        "adversarial_input.rs",
        include_str!("../adversarial_input.rs"),
    ),
    ("algebraic_law.rs", include_str!("../algebraic_law.rs")),
    (
        "all_algebraic_laws.rs",
        include_str!("../all_algebraic_laws.rs"),
    ),
    ("atomic_op.rs", include_str!("../atomic_op.rs")),
    ("bin_op.rs", include_str!("../bin_op.rs")),
    ("buffer_access.rs", include_str!("../buffer_access.rs")),
    ("by_category.rs", include_str!("../by_category.rs")),
    ("by_id.rs", include_str!("../by_id.rs")),
    (
        "catalog_is_complete.rs",
        include_str!("../catalog_is_complete.rs"),
    ),
    ("category.rs", include_str!("../category.rs")),
    ("convention.rs", include_str!("../convention.rs")),
    (
        "data_type/display.rs",
        include_str!("../data_type/display.rs"),
    ),
    (
        "data_type/layout.rs",
        include_str!("../data_type/layout.rs"),
    ),
    (
        "data_type/validation.rs",
        include_str!("../data_type/validation.rs"),
    ),
    ("data_type.rs", include_str!("../data_type.rs")),
    ("expr_variant.rs", include_str!("../expr_variant.rs")),
    (
        "engine_invariant.rs",
        include_str!("../engine_invariant.rs"),
    ),
    ("float_type.rs", include_str!("../float_type.rs")),
    ("golden_sample.rs", include_str!("../golden_sample.rs")),
    ("intrinsic_table.rs", include_str!("../intrinsic_table.rs")),
    ("invariant.rs", include_str!("../invariant.rs")),
    (
        "invariant_category.rs",
        include_str!("../invariant_category.rs"),
    ),
    ("invariants.rs", include_str!("../invariants.rs")),
    ("kat_vector.rs", include_str!("../kat_vector.rs")),
    ("law_catalog.rs", include_str!("../law_catalog.rs")),
    ("layer.rs", include_str!("../layer.rs")),
    ("lib.rs", include_str!("../lib.rs")),
    (
        "metadata_category.rs",
        include_str!("../metadata_category.rs"),
    ),
    (
        "monotonic_direction.rs",
        include_str!("../monotonic_direction.rs"),
    ),
    ("op_metadata.rs", include_str!("../op_metadata.rs")),
    ("op_signature.rs", include_str!("../op_signature.rs")),
    ("test_descriptor.rs", include_str!("../test_descriptor.rs")),
    ("tests/mod.rs", include_str!("mod.rs")),
    ("tests/source_hygiene.rs", include_str!("source_hygiene.rs")),
    (
        "tests/catalog_contracts.rs",
        include_str!("catalog_contracts.rs"),
    ),
    (
        "tests/algebra_contracts.rs",
        include_str!("algebra_contracts.rs"),
    ),
    (
        "tests/type_backend_contracts.rs",
        include_str!("type_backend_contracts.rs"),
    ),
    ("un_op.rs", include_str!("../un_op.rs")),
    ("verification.rs", include_str!("../verification.rs")),
];

const CARGO_TOML: &str = include_str!("../../Cargo.toml");

// A raw per-file line-count cap is a guideline (surfaced by the large-file
// advisory in `xtask lego-audit` and `scripts/laws/check_file_sizes.sh`), not
// a law, so no test asserts `lines < 500` here. The hygiene contracts below
// assert real properties — explicit re-exports, module docs — that hold
// regardless of file size.

#[test]
fn public_re_exports_are_explicit() {
    for (path, contents) in SOURCE_FILES {
        for (line_index, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            assert!(
                !(trimmed.starts_with("pub use ") && trimmed.contains("::*")),
                "Fix: replace glob re-export in src/{path}:{} with named re-exports",
                line_index + 1
            );
        }
    }
}

#[test]
fn module_docs_are_not_placeholders() {
    let boilerplate_doc = concat!("//! ", "Doc.");
    for (path, contents) in SOURCE_FILES {
        assert!(
            !contents.contains(boilerplate_doc),
            "Fix: replace boilerplate module docs in src/{path} with a concrete contract sentence"
        );
    }
}

#[test]
fn spec_inherits_workspace_lints_and_stays_data_only() {
    assert!(
        CARGO_TOML.lines().any(|line| line.trim() == "[lints]")
            && CARGO_TOML
                .lines()
                .any(|line| line.trim() == "workspace = true"),
        "Fix: add `[lints] workspace = true` to spec/Cargo.toml"
    );

    let forbidden = concat!("un", "safe");
    for (path, contents) in SOURCE_FILES {
        for (line_index, line) in contents.lines().enumerate() {
            assert!(
                !line
                    .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                    .any(|token| token == forbidden),
                "Fix: remove data-contract violation `{forbidden}` from src/{path}:{}",
                line_index + 1
            );
        }
    }
}
