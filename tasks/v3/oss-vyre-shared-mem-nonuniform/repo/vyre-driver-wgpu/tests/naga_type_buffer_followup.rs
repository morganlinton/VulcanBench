//! Regression tests for type and buffer-lowering follow-up findings.

use vyre_driver::DispatchConfig;
use vyre_emit_naga::program::emit_module;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, MemoryKind, Node, Program};

const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

/// Emit `program` to a naga module and assert it passes naga's full validator —
/// the same validation the wgpu backend runs before dispatch.
fn emit_validated_module(program: &Program) -> naga::Module {
    let module = emit_module(program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Fix: test program must lower to valid Naga.");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Fix: lowered test module must validate.");
    module
}

fn emit_wgsl(program: &Program) -> String {
    let module = emit_module(program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Fix: test program must lower to valid Naga.");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Fix: lowered test module must validate.");
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .expect("Fix: lowered test module must serialize to WGSL.")
}

/// `negate(u32)` must lower to a wrapping `0u - v` SUBTRACT, never `Unary(Negate)`
/// on a Uint. vyre's typecheck legalises integer `Negate` for `u32` (not raw
/// `i32`, whose `i32::MIN` overflow is rejected upstream) and the reference
/// oracle computes `0u32.wrapping_sub(v)`, but naga REJECTS unary minus on an
/// unsigned operand (`InvalidUnaryOperandType`). Before the emitter synthesized
/// `0u - v`, this exact module FAILED naga validation — so a u32 negate the
/// front-end + CPU oracle accepted could not be dispatched on the GPU (Law 10).
/// This is the CPU-side deterministic guard for that fix (the live-GPU twin is
/// `unary_int_parity::negate_u32_wraps_two_complement_on_gpu`).
#[test]
fn u32_negate_lowers_to_wrapping_subtract_and_validates() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::negate(Expr::load("a", Expr::u32(0))),
        )],
    );

    // The regression: emit_validated_module panics here if naga rejects the
    // lowering (it did, with InvalidUnaryOperandType(Negate), before the fix).
    let module = emit_validated_module(&program);

    let exprs: Vec<&naga::Expression> = module
        .entry_points
        .iter()
        .flat_map(|ep| ep.function.expressions.iter().map(|(_, e)| e))
        .collect();
    assert!(
        exprs
            .iter()
            .any(|e| matches!(e, naga::Expression::Binary { op, .. } if *op == naga::BinaryOperator::Subtract)),
        "Fix: u32 negate must lower to a `0u - v` Subtract.",
    );
    assert!(
        !exprs
            .iter()
            .any(|e| matches!(e, naga::Expression::Unary { op, .. } if *op == naga::UnaryOperator::Negate)),
        "Fix: u32 negate must NOT emit a naga Unary(Negate) (invalid on an unsigned operand).",
    );
}

#[test]
fn vec4_u32_buffers_lower_as_wgsl_vectors() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "vecs",
            0,
            BufferAccess::ReadOnly,
            DataType::Vec4U32,
        )],
        [1, 1, 1],
        vec![Node::Return],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("array<vec4<u32>>"),
        "Fix: Vec4U32 buffers must lower to vec4<u32> arrays.\n{wgsl}",
    );
}

#[test]
fn u64_buffers_lower_as_vec2_u32() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "wide",
            0,
            BufferAccess::ReadOnly,
            DataType::U64,
        )],
        [1, 1, 1],
        vec![Node::Return],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("array<vec2<u32>>"),
        "Fix: U64 buffers must lower through vec2<u32> emulation.\n{wgsl}",
    );
}

#[test]
fn bytes_buffers_fail_with_pack_prepass_error() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "bytes",
            0,
            BufferAccess::ReadOnly,
            DataType::Bytes,
        )],
        [1, 1, 1],
        vec![Node::Return],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: raw byte buffers must not lower as invalid array<u32> storage.");
    assert!(
        err.to_string().contains("pack-to-u32 pre-pass"),
        "Fix: bytes-buffer rejection must explain the required pre-pass. Got {err}",
    );
}

#[test]
fn non_word_arrays_fail_with_struct_lowering_error() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "array16",
            0,
            BufferAccess::ReadOnly,
            DataType::Array { element_size: 16 },
        )],
        [1, 1, 1],
        vec![Node::Return],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: non-4-byte arrays must not silently lower through array<u32>.");
    assert!(
        err.to_string().contains("struct-backed array"),
        "Fix: non-word array rejection must explain the struct-backed lowering requirement. Got {err}",
    );
}

#[test]
fn zero_sized_workgroup_buffers_are_rejected_at_lowering_boundary() {
    let program = Program::wrapped(
        vec![BufferDecl::workgroup("scratch", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: zero-sized workgroup buffers must not lower to Naga.");
    assert!(
        err.to_string().contains("zero static element count"),
        "Fix: zero-sized workgroup rejection must name the zero-count buffer. Got {err}",
    );
}

#[test]
fn persistent_buffers_are_rejected_before_naga_address_space_lowering() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("persist", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_kind(MemoryKind::Persistent)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: persistent buffers must be stripped before wgpu lowering.");
    assert!(
        err.to_string().contains("AsyncLoad/AsyncStore"),
        "Fix: persistent-buffer rejection must point callers at the host transfer path. Got {err}",
    );
}

#[test]
fn f16_buffers_reject_until_wgsl_parser_accepts_enable_f16() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "half",
            0,
            BufferAccess::ReadOnly,
            DataType::F16,
        )],
        [1, 1, 1],
        vec![Node::Return],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE).expect_err(
        "Fix: F16 buffers must reject before emitting WGSL this Naga stack cannot parse",
    );
    assert!(
        err.to_string().contains("enable f16"),
        "Fix: F16 rejection must name the unsupported WGSL extension. Got {err}",
    );
}
