use super::super::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_cast_u32_to_i32(v in u32_adversarial()) {
        let result = eval_cast(DataType::I32, Expr::u32(v));
        prop_assert_eq!(result, Value::I32(v as i32));
    }

    #[test]
    fn prop_cast_u32_to_bool(v in u32_adversarial()) {
        let result = eval_cast(DataType::Bool, Expr::u32(v));
        prop_assert_eq!(result, Value::Bool(v != 0));
    }

    #[test]
    fn prop_cast_i32_to_u32(v in i32_adversarial()) {
        let result = eval_cast(DataType::U32, Expr::i32(v));
        prop_assert_eq!(result, Value::U32(v as u32));
    }

    #[test]
    fn prop_cast_i32_to_bool(v in i32_adversarial()) {
        let result = eval_cast(DataType::Bool, Expr::i32(v));
        prop_assert_eq!(result, Value::Bool(v != 0));
    }

    #[test]
    fn prop_cast_f32_to_u32(v in f32_adversarial()) {
        let result = eval_cast(DataType::U32, Expr::f32(v));
        prop_assert_eq!(result, Value::U32(f64::from(canonical_f32(v)) as u32));
    }

    #[test]
    fn prop_cast_f32_to_i32(v in f32_adversarial()) {
        let result = eval_cast(DataType::I32, Expr::f32(v));
        // Interpreter path: f32 value → i32 direct truncation (WGSL
        // i32(f32) semantics). The prior path routed through u32 via
        // try_as_u32 which rejected negative / NaN inputs; adding a
        // Float arm to cast_value lets the interpreter perform the
        // conversion directly.
        let via_f32 = f64::from(canonical_f32(v)) as i32;
        prop_assert_eq!(result, Value::I32(via_f32));
    }

    #[test]
    fn prop_cast_f32_to_bool(v in f32_adversarial()) {
        let result = eval_cast(DataType::Bool, Expr::f32(v));
        prop_assert_eq!(result, Value::Bool(canonical_f32(v) != 0.0));
    }

    #[test]
    fn prop_cast_bool_to_u32(v in any::<bool>()) {
        let result = eval_cast(DataType::U32, Expr::bool(v));
        prop_assert_eq!(result, Value::U32(u32::from(v)));
    }

    #[test]
    fn prop_cast_bool_to_i32(v in any::<bool>()) {
        let result = eval_cast(DataType::I32, Expr::bool(v));
        prop_assert_eq!(result, Value::I32(i32::from(v)));
    }

    #[test]
    fn prop_cast_bool_to_f32(v in any::<bool>()) {
        let result = eval_cast(DataType::F32, Expr::bool(v));
        // bool → f32 is `true -> 1.0, false -> 0.0` (value cast,
        // not bit cast).
        prop_assert_eq!(result, Value::Float(if v { 1.0 } else { 0.0 }));
    }

    #[test]
    fn prop_cast_u32_to_f32(v in u32_adversarial()) {
        let result = eval_cast(DataType::F32, Expr::u32(v));
        // Value-preserving u32 → f32: `f32(u32_value)` per WGSL
        // semantics; the interpreter widens the result to f64 to
        // match the rest of the numeric stack.
        prop_assert_eq!(result, Value::Float(f64::from(v as f32)));
    }

    #[test]
    fn prop_cast_i32_to_f32(v in i32_adversarial()) {
        let result = eval_cast(DataType::F32, Expr::i32(v));
        prop_assert_eq!(result, Value::Float(f64::from(v as f32)));
    }
}

// ---------------------------------------------------------------------------
// Atomic – all ops on a ReadWrite buffer, monotonic increment for Add
// ---------------------------------------------------------------------------

fn eval_atomic(op: AtomicOp, buffer: &str, index: u32, expected: Option<u32>, value: u32) -> Value {
    let expr = Expr::Atomic {
        op,
        buffer: buffer.into(),
        index: Box::new(Expr::u32(index)),
        expected: expected.map(|v| Box::new(Expr::u32(v))),
        value: Box::new(Expr::u32(value)),
        ordering: MemoryOrdering::SeqCst,
    };
    eval_expr_value(&expr)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_atomic_add_monotonic(values in prop::collection::vec(any::<u32>(), 1..64)) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("counter", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("counter", Buffer::new(vec![0; 4], DataType::U32));
        let mut invocation = zero_invocation(&program);

        let mut running = 0u32;
        for v in &values {
            let old = eval_expr::eval(
                &Expr::atomic_add("counter", Expr::u32(0), Expr::u32(*v)),
                &mut invocation,
                &mut memory,
                &program,
            ).unwrap();
            prop_assert_eq!(old, Value::U32(running));
            running = running.wrapping_add(*v);
        }
        // Final buffer value must equal the accumulated sum.
        let final_val = eval_expr::eval(
            &Expr::load("counter", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(final_val, Value::U32(running));
    }

    #[test]
    fn prop_atomic_or(old in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_or("buf", Expr::u32(0), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(loaded, Value::U32(old | value));
    }

    #[test]
    fn prop_atomic_and(old in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_and("buf", Expr::u32(0), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(loaded, Value::U32(old & value));
    }

    #[test]
    fn prop_atomic_xor(old in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_xor("buf", Expr::u32(0), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(loaded, Value::U32(old ^ value));
    }

    #[test]
    fn prop_atomic_min(old in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_min("buf", Expr::u32(0), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(loaded, Value::U32(old.min(value)));
    }

    #[test]
    fn prop_atomic_max(old in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_max("buf", Expr::u32(0), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(loaded, Value::U32(old.max(value)));
    }

    #[test]
    fn prop_atomic_exchange(old in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_exchange("buf", Expr::u32(0), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(loaded, Value::U32(value));
    }

    #[test]
    fn prop_atomic_compare_exchange(old in any::<u32>(), expected in any::<u32>(), value in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(old.to_le_bytes().to_vec(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::atomic_compare_exchange("buf", Expr::u32(0), Expr::u32(expected), Expr::u32(value)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        prop_assert_eq!(result, Value::U32(old));
        let loaded = eval_expr::eval(
            &Expr::load("buf", Expr::u32(0)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        let new_val = if old == expected { value } else { old };
        prop_assert_eq!(loaded, Value::U32(new_val));
    }
}

// ---------------------------------------------------------------------------
// Load / BufLen
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_load_u32(idx in any::<u32>()) {
        let data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let program = Program::wrapped(
            vec![BufferDecl::read("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(data.clone(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::load("buf", Expr::u32(idx)),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();

        let expected = if (idx as usize) < (data.len() / 4) {
            let offset = idx as usize * 4;
            Value::U32(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]))
        } else {
            Value::U32(0)
        };
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn prop_buf_len(data in prop::collection::vec(any::<u8>(), 0..128)) {
        let program = Program::wrapped(
            vec![BufferDecl::read("buf", 0, DataType::U32)],
            [1, 1, 1],
            Vec::new(),
        );
        let mut memory = Memory::empty()
            .with_storage("buf", Buffer::new(data.clone(), DataType::U32));
        let mut invocation = zero_invocation(&program);

        let result = eval_expr::eval(
            &Expr::buf_len("buf"),
            &mut invocation,
            &mut memory,
            &program,
        ).unwrap();
        let elements = (data.len() / 4) as u32;
        prop_assert_eq!(result, Value::U32(elements));
    }
}

// ---------------------------------------------------------------------------
// Store – exercised through top-level `run`
// ---------------------------------------------------------------------------

