//! CRITIQUE_NAGA_DEEPER_2026-04-23 regression pins.
//!
//! These tests lock the four fixes that closed the silent-correctness
//! hazards in the Naga emitter. Every test would have passed on the
//! broken code by producing the WRONG output silently  -  so each one
//! must assert a specific error shape, not a general success.

use vyre::ir::{AtomicOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::memory_model::MemoryOrdering;
use vyre::DispatchConfig;
use vyre_emit_naga::program as naga_emit;

const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

fn module_to_wgsl(module: &naga::Module) -> String {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(module)
    .expect("Fix: emitted Naga module must validate before WGSL serialization");
    naga::back::wgsl::write_string(module, &info, naga::back::wgsl::WriterFlags::empty())
        .expect("Fix: validated Naga module must serialize to WGSL")
}

/// FINDING-52: when `size_bytes()` is None the emitter used to silently
/// pick 4 bytes for the array stride, producing a shader with a
/// declared stride that disagreed with the element layout. That path
/// is now rejected with a named error. We exercise the guard by way of
/// the generic "unknown stride" contract  -  a positive test here would
/// require a DataType whose `size_bytes()` returns None, which the
/// public enum does not currently expose. The guard still matters as a
/// future-proof gate. This test documents the invariant: every
/// constructable DataType used by shipped buffers MUST have a known
/// stride, and adding one that doesn't must be caught at lowering.
#[test]
fn f52_every_public_datatype_has_known_stride_today() {
    for ty in [
        DataType::Bool,
        DataType::U8,
        DataType::U16,
        DataType::U32,
        DataType::I8,
        DataType::I16,
        DataType::I32,
        DataType::F32,
        DataType::U64,
        DataType::I64,
    ] {
        assert!(
            ty.size_bytes().is_some(),
            "Fix: DataType `{ty:?}` must expose size_bytes(); the wgpu \
             emitter's array-stride guard depends on this being Some."
        );
    }
}

/// FINDING-53: `Expr::Cast { target: DataType::U64, .. }` lowers to the
/// wgpu backend's explicit `vec2<u32>` representation. Arithmetic that needs
/// carry propagation still rejects separately; widening a scalar into the pair
/// is safe and must not be deferred behind a fake "emulation pass" path.
#[test]
fn f53_cast_to_u64_lowers_to_vec2_pair() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U64),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::cast(DataType::U64, Expr::load("in", Expr::u32(0))),
        )],
    );

    let wgsl = module_to_wgsl(
        &naga_emit::emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
            .expect("Fix: U32 -> U64 cast must lower to the vec2<u32> backing representation."),
    );
    assert!(
        wgsl.contains("vec2<u32>"),
        "Fix: U64 cast must materialize the backend pair representation. WGSL: {wgsl}"
    );
}

/// FINDING-59 (CRITICAL): `BinOp::Add` on U64 operands used to be
/// lowered as componentwise vec2<u32> addition with no carry
/// propagation  -  silently wrong arithmetic. Now the gate at the top of
/// emit_binop rejects arithmetic on U64/I64 until the emulation pass
/// lands. Bitwise + equality ops remain allowed.
#[test]
fn f59_u64_add_rejects_with_named_carry_hint() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "a",
            0,
            BufferAccess::ReadWrite,
            DataType::U64,
        )],
        [1, 1, 1],
        vec![Node::store(
            "a",
            Expr::u32(0),
            Expr::add(Expr::load("a", Expr::u32(0)), Expr::load("a", Expr::u32(0))),
        )],
    );

    let err = naga_emit::emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err(
            "Fix: U64 Add must reject; a componentwise vec2<u32> sum \
             without carry would silently produce wrong results.",
        );
    let msg = format!("{err}");
    assert!(
        msg.contains("carry") && (msg.contains("U64") || msg.contains("64-bit")),
        "Fix: rejection must name the missing carry propagation so the \
         author understands the unsound backing representation; got: {msg}"
    );
}

/// FINDING-59: bitwise AND on U64 remains permitted because vec2
/// componentwise AND is mathematically correct. This test pins that
/// the rejection is scoped to unsound ops, not a blanket U64 ban.
#[test]
fn f59_u64_bitand_still_lowers() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "a",
            0,
            BufferAccess::ReadWrite,
            DataType::U64,
        )],
        [1, 1, 1],
        vec![Node::store(
            "a",
            Expr::u32(0),
            Expr::bitand(Expr::load("a", Expr::u32(0)), Expr::load("a", Expr::u32(0))),
        )],
    );

    let res = naga_emit::emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE);
    assert!(
        matches!(res, Ok(_)),
        "Fix: bitwise ops on U64 are componentwise-correct under the \
         vec2<u32> backing; rejection is a regression. Got: {:?}",
        res.err()
    );
}

#[test]
fn f_gap1_atomic_lru_update_lowers_to_timestamp_atomic_max() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("lru_slots", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "_prev",
                Expr::Atomic {
                    op: AtomicOp::LruUpdate,
                    buffer: "lru_slots".into(),
                    index: Box::new(Expr::u32(0)),
                    expected: None,
                    value: Box::new(Expr::u32(12345)),
                    ordering: MemoryOrdering::SeqCst,
                },
            ),
            Node::store("out", Expr::u32(0), Expr::u32(0)),
        ],
    );

    let wgsl = module_to_wgsl(
        &naga_emit::emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
            .expect("Fix: LRU update must lower to timestamp atomic max on wgpu."),
    );
    assert!(
        wgsl.contains("atomicMax"),
        "Fix: LRU update must lower to atomicMax timestamp semantics. WGSL: {wgsl}"
    );
}
