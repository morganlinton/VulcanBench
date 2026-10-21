use super::*;

#[test]
fn subnormal_sqrt_sin_cos_produce_canonical_results() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    assert_eq!(
        float_bits(eval_expr_value(&Expr::UnOp {
            op: UnOp::Sqrt,
            operand: Box::new(Expr::f32(pos_sub)),
        })),
        0x0000_0000
    );
    assert_eq!(
        float_bits(eval_expr_value(&Expr::UnOp {
            op: UnOp::Sqrt,
            operand: Box::new(Expr::f32(neg_sub)),
        })),
        0x8000_0000
    );
    assert_eq!(
        float_bits(eval_expr_value(&Expr::UnOp {
            op: UnOp::Sin,
            operand: Box::new(Expr::f32(pos_sub)),
        })),
        0x0000_0000
    );
    assert_eq!(
        float_bits(eval_expr_value(&Expr::UnOp {
            op: UnOp::Sin,
            operand: Box::new(Expr::f32(neg_sub)),
        })),
        0x8000_0000
    );
    assert_eq!(
        float_bits(eval_expr_value(&Expr::UnOp {
            op: UnOp::Cos,
            operand: Box::new(Expr::f32(pos_sub)),
        })),
        1.0f32.to_bits()
    );
    assert_eq!(
        float_bits(eval_expr_value(&Expr::UnOp {
            op: UnOp::Cos,
            operand: Box::new(Expr::f32(neg_sub)),
        })),
        1.0f32.to_bits()
    );
}

// ---------------------------------------------------------------------------
// 3. Atomic ops
// ---------------------------------------------------------------------------

#[test]
fn atomic_oob_index_returns_zero() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        Vec::new(),
    );
    let mut memory =
        Memory::empty().with_storage("buf", Buffer::new(vec![0xAB; 4], DataType::U32));
    let result = eval_expr::eval(
        &Expr::atomic_add("buf", Expr::u32(999), Expr::u32(1)),
        &mut zero_invocation(&program),
        &mut memory,
        &program,
    )
    .expect("Fix: OOB atomic must return zero, not panic");
    assert_eq!(result, Value::U32(0), "OOB atomic must return old=0");
}

#[test]
fn atomic_on_u64_buffer_touches_lower_half_only() {
    // The interpreter treats atomics as 4-byte ops regardless of declared element type.
    // This test documents that gap: an atomic add on a U64 buffer only modifies the
    // low 32 bits of each 64-bit slot.
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U64).with_count(1)],
        [1, 1, 1],
        Vec::new(),
    );
    let mut memory = Memory::empty().with_storage(
        "buf",
        Buffer::new(0x0000_0001_0000_0000u64.to_le_bytes().to_vec(), DataType::U64),
    );
    let old = eval_expr::eval(
        &Expr::atomic_add("buf", Expr::u32(0), Expr::u32(1)),
        &mut zero_invocation(&program),
        &mut memory,
        &program,
    )
    .expect("Fix: atomic on U64 buffer must evaluate");
    // old value read as low 32 bits
    assert_eq!(old, Value::U32(0));

    let loaded = eval_expr::eval(
        &Expr::load("buf", Expr::u32(0)),
        &mut zero_invocation(&program),
        &mut memory,
        &program,
    )
    .expect("Fix: load after atomic must succeed");
    // U64 value should now be 0x0000_0001_0000_0001
    assert_eq!(
        loaded,
        Value::U64(0x0000_0001_0000_0001),
        "atomic add on U64 must only touch lower 32 bits"
    );
}

#[test]
fn multiple_atomics_on_same_location_are_deterministic() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        Vec::new(),
    );
    let mut memory =
        Memory::empty().with_storage("buf", Buffer::new(vec![0; 4], DataType::U32));
    let mut invocation = zero_invocation(&program);

    let first = eval_expr::eval(
        &Expr::atomic_add("buf", Expr::u32(0), Expr::u32(1)),
        &mut invocation,
        &mut memory,
        &program,
    )
    .unwrap();
    let second = eval_expr::eval(
        &Expr::atomic_add("buf", Expr::u32(0), Expr::u32(1)),
        &mut invocation,
        &mut memory,
        &program,
    )
    .unwrap();

    assert_eq!(first, Value::U32(0), "first atomic must see old=0");
    assert_eq!(second, Value::U32(1), "second atomic must see old=1");

    let final_val = eval_expr::eval(
        &Expr::load("buf", Expr::u32(0)),
        &mut invocation,
        &mut memory,
        &program,
    )
    .unwrap();
    assert_eq!(final_val, Value::U32(2));
}

// ---------------------------------------------------------------------------
// 4. Buffer access
// ---------------------------------------------------------------------------

#[test]
fn oob_load_returns_zero() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(999)),
        )],
    );
    let outputs = reference_eval(
        &program,
        &[Value::from(vec![0xAB; 4]), Value::from(vec![0u8; 4])],
    )
    .expect("Fix: OOB load must not panic");
    assert_eq!(
        outputs[0].to_bytes(),
        vec![0; 4],
        "OOB load must return defined-type zero"
    );
}

#[test]
fn oob_store_is_silent_noop() {
    // Validation rejects a constant OOB index (V036), so load the index
    // dynamically from a buffer to force runtime evaluation.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("idx", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::load("idx", Expr::u32(0)),
            Expr::u32(0xDEAD_BEEF),
        )],
    );
    let outputs = reference_eval(
        &program,
        &[
            Value::from(999u32.to_le_bytes().to_vec()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("Fix: OOB store must not panic");
    assert_eq!(outputs[0].to_bytes(), vec![0; 4], "OOB store must be silent no-op");
}

#[test]
fn zero_sized_buffer_load_returns_zero() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32).with_count(0),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(0)),
        )],
    );
    let outputs = reference_eval(
        &program,
        &[Value::from(vec![]), Value::from(vec![0u8; 4])],
    )
    .expect("Fix: zero-sized buffer load must not panic");
    assert_eq!(
        outputs[0].to_bytes(),
        vec![0; 4],
        "load from zero-sized buffer must return zero"
    );
}

#[test]
fn zero_sized_buffer_store_is_noop() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(0)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::u32(0xDEAD_BEEF),
        )],
    );
    let outputs = reference_eval(&program, &[Value::from(vec![])])
        .expect("Fix: zero-sized buffer store must not panic");
    assert_eq!(
        outputs.len(),
        1,
        "zero-sized output buffer is still declared as an output"
    );
    assert_eq!(
        outputs[0].to_bytes(),
        Vec::<u8>::new(),
        "zero-sized output must yield empty bytes"
    );
}

#[test]
fn u32_max_index_load_returns_zero() {
    // u32::MAX as an index triggers offset overflow in byte_offset,
    // which the interpreter treats as OOB and returns zero.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(u32::MAX)),
        )],
    );
    let outputs = reference_eval(
        &program,
        &[Value::from(vec![0xAB; 4]), Value::from(vec![0u8; 4])],
    )
    .expect("Fix: u32::MAX index load must not panic");
    assert_eq!(
        outputs[0].to_bytes(),
        vec![0; 4],
        "u32::MAX index load must return zero"
    );
}
