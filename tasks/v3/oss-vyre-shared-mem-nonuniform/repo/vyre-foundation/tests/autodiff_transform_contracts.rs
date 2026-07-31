//! Contract tests for RFC-0002 autodiff as an IR transform.

#[path = "__split/autodiff_transform_contracts_support.rs"]
mod autodiff_transform_contracts_support;

use autodiff_transform_contracts_support::{
    flatten_nodes, generated_cast_program, generated_differentiable_program,
    generated_f32_identity_cast_program, generated_intermediate_buffer_program,
    generated_nondifferentiable_cast_shape, square_via_local_program,
};
use vyre_foundation::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::transform::autodiff::{grad, grad_with_pullback, AutodiffError};
use vyre_foundation::validate::validate;

#[test]
fn grad_with_pullback_is_public_and_populates_top_level_nodes() {
    let (backward, pullbacks) = grad_with_pullback(&square_via_local_program(), &["out"], &["x"])
        .expect("square program must be differentiable");

    assert_eq!(pullbacks.len(), 2);
    assert!(
        pullbacks.contains_key(&0) && pullbacks.contains_key(&1),
        "Let y and Store out both need pullback metadata"
    );

    let names = backward
        .buffers()
        .iter()
        .map(|buffer| buffer.name())
        .collect::<Vec<_>>();
    assert!(names.contains(&"grad_out"));
    assert!(names.contains(&"grad_x"));
}

#[test]
fn generated_reverse_program_declares_adjoint_before_assignment_and_validates() {
    let backward = grad(&square_via_local_program(), &["out"], &["x"])
        .expect("square program must be differentiable");
    let errors = validate(&backward);
    assert!(
        errors.is_empty(),
        "generated autodiff program must validate, got: {:?}",
        errors
            .iter()
            .map(|error| error.message())
            .collect::<Vec<_>>()
    );

    let flattened = flatten_nodes(backward.entry());
    let let_index = flattened
        .iter()
        .position(
            |node| matches!(node, Node::Let { name, .. } if name.as_str().starts_with("_adj_y_")),
        )
        .expect("reverse program must predeclare local adjoint for y");
    let assign_index = flattened
        .iter()
        .position(|node| matches!(node, Node::Assign { name, .. } if name.as_str().starts_with("_adj_y_")))
        .expect("reverse program must accumulate into local adjoint for y");
    assert!(
        let_index < assign_index,
        "local adjoint declaration must precede downstream reverse accumulation"
    );
}

#[test]
fn gradient_buffers_are_backend_allocated_outputs_not_required_inputs() {
    let backward = grad(&square_via_local_program(), &["out"], &["x"])
        .expect("square program must be differentiable");

    for buffer in backward
        .buffers()
        .iter()
        .filter(|buffer| buffer.name().starts_with("grad_"))
    {
        assert!(
            buffer.is_pipeline_live_out() && !buffer.is_output(),
            "{} must be a pipeline live-out, not a second BufferDecl::output, so multi-gradient Programs validate while backends allocate fresh storage",
            buffer.name()
        );
        assert_eq!(buffer.element(), DataType::F32);
    }
}

#[test]
fn generated_autodiff_matrix_validates_for_arithmetic_select_and_fma_shapes() {
    for seed in 0..4096u32 {
        let forward = generated_differentiable_program(seed);
        let (backward, pullbacks) = grad_with_pullback(&forward, &["out"], &["x", "w"])
            .expect("generated differentiable program must autodiff");

        assert!(
            !pullbacks.is_empty(),
            "Fix: generated autodiff pullback metadata must not be empty for seed {seed}."
        );
        let errors = validate(&backward);
        assert!(
            errors.is_empty(),
            "Fix: generated autodiff backward program for seed {seed} must validate, got {:?}",
            errors
                .iter()
                .map(|error| error.message())
                .collect::<Vec<_>>()
        );

        let buffers = backward
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>();
        assert!(buffers.contains(&"grad_out"), "seed {seed}");
        assert!(buffers.contains(&"grad_x"), "seed {seed}");
        assert!(buffers.contains(&"grad_w"), "seed {seed}");

        let flattened = flatten_nodes(backward.entry());
        for grad_name in ["grad_out", "grad_x", "grad_w"] {
            let clear_index = flattened
                .iter()
                .position(|node| {
                    matches!(
                        node,
                        Node::Store { buffer, value, .. }
                            if buffer.as_str() == grad_name && matches!(value, Expr::LitF32(0.0))
                    )
                })
                .expect("generated backward program must clear every gradient buffer");
            let later_write = flattened.iter().enumerate().any(|(index, node)| {
                index > clear_index
                    && matches!(
                        node,
                        Node::Store { buffer, .. } if buffer.as_str() == grad_name
                    )
            });
            assert!(
                later_write,
                "Fix: generated backward program must write {grad_name} after clearing it for seed {seed}."
            );
        }
    }
}

#[test]
fn generated_autodiff_accepts_only_f32_identity_cast_gradient_paths() {
    for seed in 0..4096u32 {
        let forward = generated_f32_identity_cast_program(seed);
        let backward = grad(&forward, &["out"], &["x", "w"])
            .expect("f32-to-f32 identity casts must preserve differentiability");
        let errors = validate(&backward);
        assert!(
            errors.is_empty(),
            "Fix: f32 identity cast autodiff program for seed {seed} must validate, got {:?}",
            errors
                .iter()
                .map(|error| error.message())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn generated_autodiff_rejects_integer_and_bool_cast_gradient_paths() {
    for seed in 0..4096u32 {
        let (source, target) = generated_nondifferentiable_cast_shape(seed);
        let program = generated_cast_program(source, target);
        let error = grad(&program, &["out"], &["x"])
            .expect_err("non-f32 cast gradient paths must be rejected by autodiff");
        match error {
            AutodiffError::NotDifferentiable { op, fix } => {
                assert!(
                    op.contains("Expr::Cast"),
                    "Fix: cast rejection for seed {seed} must name Expr::Cast, got {op}."
                );
                assert!(
                    fix.contains("f32-to-f32 identity"),
                    "Fix: cast rejection for seed {seed} must explain the differentiable cast rule, got {fix}."
                );
            }
            other => panic!("expected NotDifferentiable for non-f32 cast seed {seed}, got {other}"),
        }
    }
}

#[test]
fn generated_autodiff_propagates_through_f32_intermediate_buffers() {
    for seed in 0..2048u32 {
        let forward = generated_intermediate_buffer_program(seed);
        let backward = grad(&forward, &["out"], &["x"])
            .expect("f32 intermediate buffer paths must be differentiable");
        let errors = validate(&backward);
        assert!(
            errors.is_empty(),
            "Fix: intermediate-buffer autodiff program for seed {seed} must validate, got {:?}",
            errors
                .iter()
                .map(|error| error.message())
                .collect::<Vec<_>>()
        );

        let buffers = backward
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>();
        assert!(
            buffers.contains(&"grad_tmp"),
            "Fix: backward program for seed {seed} must declare grad_tmp scratch for f32 intermediate memory."
        );
        let grad_tmp = backward
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "grad_tmp")
            .expect("grad_tmp must be declared");
        assert!(
            !grad_tmp.is_pipeline_live_out(),
            "Fix: grad_tmp for seed {seed} is internal adjoint scratch and must not be exported as a pipeline output."
        );

        let flattened = flatten_nodes(backward.entry());
        let accumulates_tmp = flattened.iter().any(|node| {
            matches!(
                node,
                Node::Store { buffer, value, .. }
                    if buffer.as_str() == "grad_tmp" && matches!(value, Expr::BinOp { op: BinOp::Add, .. })
            )
        });
        let clears_tmp = flattened.iter().any(|node| {
            matches!(
                node,
                Node::Store { buffer, value, .. }
                    if buffer.as_str() == "grad_tmp" && matches!(value, Expr::LitF32(0.0))
            )
        });
        assert!(
            accumulates_tmp,
            "Fix: backward program for seed {seed} must accumulate adjoints from tmp loads into grad_tmp."
        );
        assert!(
            clears_tmp,
            "Fix: backward program for seed {seed} must clear grad_tmp after consuming a forward tmp store."
        );
    }
}

#[test]
fn generated_autodiff_rejects_integer_buffer_load_gradient_paths() {
    for seed in 0..1024u32 {
        let source = match seed % 3 {
            0 => DataType::U32,
            1 => DataType::I32,
            _ => DataType::Bool,
        };
        let forward = Program::wrapped(
            vec![
                BufferDecl::storage("x", 0, BufferAccess::ReadOnly, source).with_count(1),
                BufferDecl::output("out", 1, DataType::F32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::load("x", Expr::u32(0)),
            )],
        );

        let error = grad(&forward, &["out"], &["x"])
            .expect_err("discrete buffer loads must not silently receive f32 adjoints");
        match error {
            AutodiffError::NotDifferentiable { op, fix } => {
                assert!(
                    op.contains("Expr::Load"),
                    "Fix: integer/bool load rejection for seed {seed} must name Expr::Load, got {op}."
                );
                assert!(
                    fix.contains("f32 buffer loads"),
                    "Fix: integer/bool load rejection for seed {seed} must explain the f32 memory-gradient rule, got {fix}."
                );
            }
            other => {
                panic!("expected NotDifferentiable for integer/bool load seed {seed}, got {other}")
            }
        }
    }
}

#[test]
fn generated_autodiff_rejects_nondifferentiable_bitwise_shapes() {
    for seed in 0..1024u32 {
        let op = match seed % 3 {
            0 => BinOp::BitAnd,
            1 => BinOp::BitOr,
            _ => BinOp::BitXor,
        };
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::BinOp {
                    op,
                    left: Box::new(Expr::load("x", Expr::u32(0))),
                    right: Box::new(Expr::u32(seed)),
                },
            )],
        );

        let error = grad(&program, &["out"], &["x"])
            .expect_err("generated bitwise programs must be rejected by autodiff");
        assert!(
            matches!(error, AutodiffError::NotDifferentiable { .. }),
            "Fix: generated bitwise autodiff rejection must be NotDifferentiable for seed {seed}, got {error}."
        );
    }
}

#[test]
fn bitwise_path_still_fails_with_actionable_not_differentiable_error() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::BinOp {
                op: BinOp::BitAnd,
                left: Box::new(Expr::load("x", Expr::u32(0))),
                right: Box::new(Expr::u32(0xff)),
            },
        )],
    );

    let error = grad(&program, &["out"], &["x"]).expect_err("bitwise path is not differentiable");
    match error {
        AutodiffError::NotDifferentiable { op, fix } => {
            assert!(op.contains("BitAnd"));
            assert!(fix.contains("differentiable"));
        }
        other => panic!("expected NotDifferentiable, got {other}"),
    }
}
