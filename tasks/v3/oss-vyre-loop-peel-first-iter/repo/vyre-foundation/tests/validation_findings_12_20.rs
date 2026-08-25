//! Regression coverage for F-IR-12..20 validator findings.

use std::sync::Arc;

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::serial::wire::MAX_PROGRAM_BYTES;
use vyre_foundation::validate::{
    validate, validate_with_options, BackendValidationCapabilities, ValidationOptions,
    DEFAULT_MAX_EXPR_DEPTH,
};

struct ScalarOnlyBackend;

impl BackendValidationCapabilities for ScalarOnlyBackend {
    fn backend_name(&self) -> &'static str {
        "scalar-only"
    }

    fn supports_cast_target(&self, target: &DataType) -> bool {
        matches!(target, DataType::U32 | DataType::I32 | DataType::Bool)
    }
}

fn output_program(nodes: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        nodes,
    )
}

#[test]
fn backend_specific_cast_target_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "tmp",
        Expr::cast(DataType::U16, Expr::u32(7)),
    )]);

    let report = validate_with_options(
        &program,
        ValidationOptions::default().with_backend(&ScalarOnlyBackend),
    );
    assert!(
        report.errors.iter().any(|error| error
            .message()
            .contains("does not support cast target `u16`")),
        "backend-specific cast target must be rejected, got {:?}",
        report.errors
    );

    let valid = output_program(vec![Node::let_bind(
        "tmp",
        Expr::cast(DataType::U32, Expr::u32(7)),
    )]);
    assert!(
        validate_with_options(
            &valid,
            ValidationOptions::default().with_backend(&ScalarOnlyBackend)
        )
        .errors
        .is_empty(),
        "supported backend cast target must pass validation"
    );
}

#[test]
fn atomic_read_only_buffer_is_rejected_and_read_write_passes() {
    let invalid = Program::wrapped(
        vec![BufferDecl::read("src", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::let_bind(
            "tmp",
            Expr::atomic_add("src", Expr::u32(0), Expr::u32(1)),
        )],
    );
    let errors = validate(&invalid);
    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("atomic `Add` targets read-only buffer `src`")
        }),
        "read-only atomic must be rejected with buffer/op context, got {:?}",
        errors
    );

    let valid = Program::wrapped(
        vec![BufferDecl::read_write("dst", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::let_bind(
            "tmp",
            Expr::atomic_add("dst", Expr::u32(0), Expr::u32(1)),
        )],
    );
    assert!(
        validate(&valid).is_empty(),
        "read-write atomic must pass validation"
    );
}

#[test]
fn sibling_duplicate_lets_are_rejected_even_when_shadowing_is_allowed() {
    let invalid = output_program(vec![
        Node::let_bind("dup", Expr::u32(1)),
        Node::let_bind("dup", Expr::u32(2)),
    ]);
    let report = validate_with_options(&invalid, ValidationOptions::default().with_shadowing(true));
    assert!(
        report.errors.iter().any(|error| error
            .message()
            .contains("duplicate sibling let binding `dup`")),
        "same-region duplicate let must be rejected, got {:?}",
        report.errors
    );

    let valid = output_program(vec![
        Node::let_bind("lhs", Expr::u32(1)),
        Node::let_bind("rhs", Expr::u32(2)),
    ]);
    assert!(
        validate(&valid).is_empty(),
        "distinct sibling lets must pass"
    );
}

#[test]
fn constant_store_index_overflow_is_rejected() {
    let invalid = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("buf", Expr::u32(4), Expr::u32(1))],
    );
    let errors = validate(&invalid);
    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("store index 4 overflows buffer `buf` with count 4")
        }),
        "constant out-of-bounds store must be rejected, got {:?}",
        errors
    );

    let valid = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("buf", Expr::u32(3), Expr::u32(1))],
    );
    assert!(validate(&valid).is_empty(), "in-bounds store must pass");
}

#[test]
fn expression_depth_cap_rejects_pathological_nesting() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut deep = Expr::u32(0);
            for _ in 0..=DEFAULT_MAX_EXPR_DEPTH {
                deep = Expr::add(Expr::u32(1), deep);
            }
            let invalid = output_program(vec![Node::let_bind("deep", deep)]);
            let errors = validate(&invalid);
            let overflow_depth = DEFAULT_MAX_EXPR_DEPTH + 1;
            assert!(
                errors.iter().any(|error| {
                    error.message().contains(&format!(
                        "expression nesting depth {overflow_depth} exceeds max {DEFAULT_MAX_EXPR_DEPTH}"
                    ))
                }),
                "expression depth overflow must be rejected, got {:?}",
                errors
            );

            let mut shallow = Expr::u32(0);
            for _ in 0..DEFAULT_MAX_EXPR_DEPTH {
                shallow = Expr::add(Expr::u32(1), shallow);
            }
            let valid = output_program(vec![Node::let_bind("ok", shallow)]);
            assert!(
                validate(&valid).is_empty(),
                "depth-{DEFAULT_MAX_EXPR_DEPTH} expression must pass"
            );
        })
        .expect("spawn depth test worker")
        .join()
        .expect("depth test worker panicked");
}

#[test]
fn framing_rejects_wire_blobs_over_64_mb() {
    let oversized = vec![0_u8; MAX_PROGRAM_BYTES + 1];
    let error = Program::from_wire(&oversized).expect_err("oversized blob must fail");
    assert!(
        error
            .to_string()
            .contains("exceeding the 67108864-byte IR framing cap"),
        "oversized wire blob must be rejected at framing, got {error}"
    );

    let valid = output_program(vec![Node::store("out", Expr::u32(0), Expr::u32(1))]);
    let bytes = valid.to_wire().expect("valid program must encode");
    let decoded = Program::from_wire(&bytes).expect("small blob must decode");
    assert!(
        validate(&decoded).is_empty(),
        "decoded small blob must validate"
    );
}

#[test]
fn nested_shadowing_is_rejected_by_default_and_opt_in_can_allow_it() {
    let invalid = output_program(vec![
        Node::let_bind("acc", Expr::u32(1)),
        Node::Region {
            generator: "test.region".into(),
            source_region: None,
            body: Arc::new(vec![Node::let_bind("acc", Expr::u32(2))]),
        },
    ]);
    let errors = validate(&invalid);
    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("duplicate local binding `acc` shadows an outer scope")
        }),
        "nested shadowing must be rejected by default, got {:?}",
        errors
    );

    let report = validate_with_options(&invalid, ValidationOptions::default().with_shadowing(true));
    assert!(
        report.errors.is_empty(),
        "explicit shadowing opt-in must allow nested shadowing"
    );
}

#[test]
fn narrowing_cast_emits_warning_without_rejecting_program() {
    let warning_program = output_program(vec![Node::let_bind(
        "tmp",
        Expr::cast(DataType::U8, Expr::u32(255)),
    )]);
    let report = validate_with_options(&warning_program, ValidationOptions::default());
    assert!(
        report.errors.is_empty(),
        "narrowing cast warning must not reject the program: {:?}",
        report.errors
    );
    assert!(
        report.warnings.iter().any(|warning| warning
            .message()
            .contains("narrowing cast from `u32` to `u8`")),
        "narrowing cast must emit a warning, got {:?}",
        report.warnings
    );

    let ok_program = output_program(vec![Node::let_bind(
        "tmp",
        Expr::cast(DataType::U64, Expr::u32(255)),
    )]);
    let ok_report = validate_with_options(&ok_program, ValidationOptions::default());
    assert!(
        ok_report.errors.is_empty() && ok_report.warnings.is_empty(),
        "widening cast must pass without warnings, got {:?} / {:?}",
        ok_report.errors,
        ok_report.warnings
    );
}
