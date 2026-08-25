use super::super::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_store_u32_roundtrip(value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::store("out", Expr::u32(0), Expr::u32(value)),
            ],
        );
        let inputs = [Value::from(vec![0; 4])];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: store program must execute successfully");
        prop_assert_eq!(outputs.len(), 1);
        let bytes = outputs[0].to_bytes();
        prop_assert_eq!(bytes, value.to_le_bytes().to_vec());
    }

    #[test]
    fn prop_store_oob_is_silent_noop(index in 1u32..) {
        // Store past the end of a 1-element buffer must not panic or error.
        // Use a runtime-loaded index so this exercises OOB store semantics
        // instead of the validator's constant-index rejection.
        let program = Program::wrapped(
            vec![
                BufferDecl::read("idx", 0, DataType::U32).with_count(1),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![
                Node::store("out", Expr::load("idx", Expr::u32(0)), Expr::u32(0xDEADBEEF)),
            ],
        );
        let inputs = [Value::from(index.to_le_bytes().to_vec()), Value::from(vec![0; 4])];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: OOB store must be a silent no-op");
        prop_assert_eq!(outputs[0].to_bytes(), vec![0; 4]);
    }
}

// ---------------------------------------------------------------------------
// Call – primitive.compare.select
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Expr::Select replaces the deprecated `primitive.compare.select` op
    /// id used in earlier drafts of this test file. The semantics are the
    /// same: `cond != 0 ? value : 0`, but Select is a first-class IR
    /// variant with a direct evaluator, no call-inlining required.
    #[test]
    fn prop_select_matches_conditional(value in any::<u32>(), condition in any::<u32>()) {
        let program = empty_program();
        let expr = Expr::Select {
            cond: Box::new(Expr::BinOp {
                op: vyre::ir::BinOp::Ne,
                left: Box::new(Expr::u32(condition)),
                right: Box::new(Expr::u32(0)),
            }),
            true_val: Box::new(Expr::u32(value)),
            false_val: Box::new(Expr::u32(0)),
        };
        let result = eval_expr::eval(&expr, &mut zero_invocation(&program), &mut Memory::empty(), &program)
            .expect("Fix: Expr::Select must evaluate");
        let expected = if condition != 0 { Value::U32(value) } else { Value::U32(0) };
        prop_assert_eq!(result, expected);
    }
}

// ---------------------------------------------------------------------------
// Opaque – must produce actionable error
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_opaque_errors_actionably(_dummy in any::<u32>()) {
        let program = empty_program();
        let expr = Expr::opaque(DummyOpaque);
        let result = eval_expr::eval(&expr, &mut zero_invocation(&program), &mut Memory::empty(), &program);
        match result {
            Err(e) => {
                let msg = e.to_string();
                prop_assert!(
                    msg.contains("Fix:"),
                    "opaque error must contain actionable 'Fix:' hint, got: {msg}"
                );
                prop_assert!(
                    msg.contains("does not support opaque expression"),
                    "opaque error must mention unsupported opaque expression, got: {msg}"
                );
            }
            Ok(v) => prop_assert!(false, "opaque expression must error, got: {v:?}"),
        }
    }
}
