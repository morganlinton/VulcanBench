use super::*;

#[test]
fn unop_logical_not_on_f32_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::UnOp {
            op: UnOp::LogicalNot,
            operand: Box::new(Expr::LitF32(1.0)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("operand has type `f32`")),
        "LogicalNot on f32 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn invocation_id_axis_out_of_range_is_rejected() {
    let program = output_program(vec![Node::let_bind("x", Expr::InvocationId { axis: 3 })]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("invocation/workgroup ID axis 3 out of range")),
        "axis=3 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn workgroup_id_axis_out_of_range_is_rejected() {
    let program = output_program(vec![Node::let_bind("x", Expr::WorkgroupId { axis: 5 })]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("invocation/workgroup ID axis 5 out of range")),
        "axis=5 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn return_not_at_end_of_scope_is_rejected() {
    let program = output_program(vec![
        Node::return_(),
        Node::store("out", Expr::u32(0), Expr::u32(1)),
    ]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("unreachable statements after `return`")),
        "statements after return must be rejected, got {:?}",
        errors
    );
}

#[test]
fn store_type_mismatch_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("Node::Store buffer `buf` value has type `u32` but element type is `f32`")),
        "store type mismatch must be rejected, got {:?}",
        errors
    );
}

#[test]
fn atomic_on_non_u32_element_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::Add,
                buffer: "buf".into(),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(1)),
                ordering: MemoryOrdering::SeqCst,
            },
        )],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("atomic on buffer `buf` with non-u32 element type `f32`")),
        "atomic on f32 buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn atomic_compare_exchange_missing_expected_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::CompareExchange,
                buffer: "buf".into(),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(1)),
                ordering: MemoryOrdering::SeqCst,
            },
        )],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("compare-exchange atomic is missing expected value")),
        "cmpxchg without expected must be rejected, got {:?}",
        errors
    );
}

#[test]
fn non_compare_exchange_atomic_with_expected_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::Add,
                buffer: "buf".into(),
                index: Box::new(Expr::u32(0)),
                expected: Some(Box::new(Expr::u32(0))),
                value: Box::new(Expr::u32(1)),
                ordering: MemoryOrdering::SeqCst,
            },
        )],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("non-compare-exchange atomic includes an expected value")),
        "add atomic with expected must be rejected, got {:?}",
        errors
    );
}

#[test]
fn mod_with_non_u32_operand_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::BinOp {
            op: BinOp::Mod,
            left: Box::new(Expr::LitF32(1.0)),
            right: Box::new(Expr::u32(2)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("binary operation `Mod` left operand must be `u32` or `i32`, got `f32`")),
        "Mod with f32 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn bitwise_op_with_mismatched_integer_types_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::BinOp {
            op: BinOp::BitAnd,
            left: Box::new(Expr::u32(1)),
            right: Box::new(Expr::LitI32(2)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("operands have mismatched integer types: left=`u32`, right=`i32`")),
        "BitAnd with u32+i32 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn shift_with_non_u32_operand_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::BinOp {
            op: BinOp::Shl,
            left: Box::new(Expr::LitI32(1)),
            right: Box::new(Expr::u32(2)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("binary operation `Shl` left operand has type `i32`")),
        "Shl with i32 left must be rejected, got {:?}",
        errors
    );
}

#[test]
fn comparison_with_mismatched_types_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::BinOp {
            op: BinOp::Eq,
            left: Box::new(Expr::u32(1)),
            right: Box::new(Expr::LitF32(1.0)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("binary comparison `Eq` operands have mismatched types")),
        "Eq with u32+f32 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn var_of_undeclared_name_is_rejected() {
    let program = output_program(vec![Node::let_bind("x", Expr::var("y"))]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("reference to undeclared variable `y`")),
        "undeclared variable must be rejected, got {:?}",
        errors
    );
}

#[test]
fn opaque_node_with_empty_extension_kind_is_rejected() {
    use vyre_foundation::ir::NodeExtension;

    #[derive(Debug)]
    struct BadExtension;

    impl NodeExtension for BadExtension {
        fn extension_kind(&self) -> &'static str {
            ""
        }
        fn debug_identity(&self) -> &str {
            "bad"
        }
        fn stable_fingerprint(&self) -> [u8; 32] {
            [0; 32]
        }
        fn validate_extension(&self) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let program = output_program(vec![Node::opaque(BadExtension)]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("opaque node extension has an empty extension_kind")),
        "opaque node with empty kind must be rejected, got {:?}",
        errors
    );
}

#[test]
fn opaque_node_with_empty_debug_identity_is_rejected() {
    use vyre_foundation::ir::NodeExtension;

    #[derive(Debug)]
    struct BadExtension;

    impl NodeExtension for BadExtension {
        fn extension_kind(&self) -> &'static str {
            "test.ext"
        }
        fn debug_identity(&self) -> &str {
            ""
        }
        fn stable_fingerprint(&self) -> [u8; 32] {
            [0; 32]
        }
        fn validate_extension(&self) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let program = output_program(vec![Node::opaque(BadExtension)]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("opaque node extension `test.ext` has an empty debug_identity")),
        "opaque node with empty identity must be rejected, got {:?}",
        errors
    );
}
