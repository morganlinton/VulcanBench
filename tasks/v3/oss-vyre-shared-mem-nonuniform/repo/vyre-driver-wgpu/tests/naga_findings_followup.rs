//! Regression tests for the post-audit Naga lowering follow-up.

use vyre_driver::DispatchConfig;
use vyre_emit_naga::program::emit_module;
use vyre_foundation::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

fn emit_wgsl(program: &Program) -> String {
    let module = emit_module(program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Fix: test program must lower to a valid Naga module.");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Fix: test program must validate after lowering.");
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .expect("Fix: lowered module must serialize to WGSL.")
}

#[test]
fn integer_if_conditions_are_coerced_to_bool() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::load("input", Expr::u32(0)),
            vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
        )],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("if (") && wgsl.contains("!= 0u)"),
        "Fix: integer predicates must lower through `!= 0`, not invalid raw u32 conditions.\n{wgsl}",
    );
}

#[test]
fn loop_bound_side_effect_is_hoisted_once() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("counter", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::atomic_add("counter", Expr::u32(0), Expr::u32(1)),
                vec![Node::store("out", Expr::u32(0), Expr::var("i"))],
            ),
            Node::Return,
        ],
    );

    let wgsl = emit_wgsl(&program);
    let atomic_adds = wgsl.matches("atomicAdd").count();
    assert_eq!(
        atomic_adds, 1,
        "Fix: loop bounds with side effects must be hoisted once, not re-emitted in guard/continuing.\n{wgsl}",
    );
}

#[test]
fn buf_len_on_workgroup_buffer_lowers_to_static_count() {
    let program = Program::wrapped(
        vec![
            BufferDecl::workgroup("scratch", 4, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::BufLen {
                buffer: "scratch".into(),
            },
        )],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("4u"),
        "Fix: BufLen over workgroup memory must lower to the static BufferDecl::workgroup count, not invalid arrayLength(&workgroup_array).\n{wgsl}",
    );
    assert!(
        !wgsl.contains("arrayLength(&scratch"),
        "Fix: Naga cannot emit arrayLength for workgroup arrays; this path must remain static.\n{wgsl}",
    );
}

#[test]
fn fma_rejects_non_f32_operands_with_actionable_message() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind(
            "bad_fma",
            Expr::Fma {
                a: Box::new(Expr::u32(1)),
                b: Box::new(Expr::u32(2)),
                c: Box::new(Expr::u32(3)),
            },
        )],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: Fma with integer operands must reject before emitting invalid Naga.");
    let message = err.to_string();
    assert!(
        message.contains("Fma requires three f32 operands") && message.contains("Fix:"),
        "Fix: Fma dtype rejection must name the f32-only contract and remediation. Got {message}",
    );
}

#[test]
fn f32_fma_lowers_to_naga_math_fma() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::Fma {
                a: Box::new(Expr::LitF32(2.0)),
                b: Box::new(Expr::LitF32(3.0)),
                c: Box::new(Expr::LitF32(4.0)),
            },
        )],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("fma(") || wgsl.contains("10f"),
        "Fix: f32 Fma must lower through Naga MathFunction::Fma or equivalent constant-folded value.\n{wgsl}",
    );
}

#[test]
fn unary_unpack_ops_are_lowered() {
    let input = Expr::load("packed", Expr::u32(0));
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("packed", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("unpack4_low", 1, DataType::U32),
            BufferDecl::output("unpack4_high", 2, DataType::U32),
            BufferDecl::output("unpack8_low", 3, DataType::U32),
            BufferDecl::output("unpack8_high", 4, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::store(
                "unpack4_low",
                Expr::u32(0),
                Expr::UnOp {
                    op: UnOp::Unpack4Low,
                    operand: Box::new(input.clone()),
                },
            ),
            Node::store(
                "unpack4_high",
                Expr::u32(0),
                Expr::UnOp {
                    op: UnOp::Unpack4High,
                    operand: Box::new(input.clone()),
                },
            ),
            Node::store(
                "unpack8_low",
                Expr::u32(0),
                Expr::UnOp {
                    op: UnOp::Unpack8Low,
                    operand: Box::new(input.clone()),
                },
            ),
            Node::store(
                "unpack8_high",
                Expr::u32(0),
                Expr::UnOp {
                    op: UnOp::Unpack8High,
                    operand: Box::new(input),
                },
            ),
        ],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("0x") || wgsl.contains("&"),
        "Fix: unpack unary ops should lower to explicit bitfield masking/shifting.\n{wgsl}",
    );
}

#[test]
fn subgroup_binops_are_lowered_as_subgroup_statements() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("subgroup_shuffle", 1, DataType::U32),
            BufferDecl::output("subgroup_ballot", 2, DataType::U32),
            BufferDecl::output("subgroup_reduce", 3, DataType::U32),
            BufferDecl::output("subgroup_broadcast", 4, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::store(
                "subgroup_shuffle",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::Shuffle,
                    left: Box::new(Expr::u32(0x5)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
            Node::store(
                "subgroup_ballot",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::Ballot,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
            Node::store(
                "subgroup_reduce",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::WaveReduce,
                    left: Box::new(Expr::u32(7)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
            Node::store(
                "subgroup_broadcast",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::WaveBroadcast,
                    left: Box::new(Expr::u32(4)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
        ],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("subgroup"),
        "Fix: subgroup BinOps must lower through subgroup statements, not generic BinaryOperator emission.\n{wgsl}",
    );
}

/// Each subgroup BinOp must lower to its OWN distinct WGSL intrinsic, not a
/// catch-all. In particular `WaveBroadcast` must emit `subgroupBroadcast` (gather
/// from a uniform lane), NOT `subgroupShuffle` — collapsing broadcast into
/// shuffle would be a silent semantic substitution (broadcast requires a uniform
/// source lane; shuffle allows per-lane).
#[test]
fn subgroup_binops_lower_to_distinct_intrinsics() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("subgroup_shuffle", 1, DataType::U32),
            BufferDecl::output("subgroup_ballot", 2, DataType::U32),
            BufferDecl::output("subgroup_reduce", 3, DataType::U32),
            BufferDecl::output("subgroup_broadcast", 4, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::store(
                "subgroup_shuffle",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::Shuffle,
                    left: Box::new(Expr::u32(0x5)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
            Node::store(
                "subgroup_ballot",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::Ballot,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
            Node::store(
                "subgroup_reduce",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::WaveReduce,
                    left: Box::new(Expr::u32(7)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
            Node::store(
                "subgroup_broadcast",
                Expr::u32(0),
                Expr::BinOp {
                    op: BinOp::WaveBroadcast,
                    left: Box::new(Expr::u32(4)),
                    right: Box::new(Expr::u32(0)),
                },
            ),
        ],
    );

    let wgsl = emit_wgsl(&program);
    for intrinsic in [
        "subgroupShuffle(",
        "subgroupBallot(",
        "subgroupAdd(",
        "subgroupBroadcast(",
    ] {
        assert!(
            wgsl.contains(intrinsic),
            "Fix: subgroup BinOp must emit `{intrinsic}`; each spelling maps to its own intrinsic.\n{wgsl}",
        );
    }
    // Ballot's u32 predicate must be coerced to a bool before subgroupBallot
    // (naga rejects a non-bool predicate), so the comparison must appear.
    assert!(
        wgsl.contains("!= 0u") || wgsl.contains("!=0u"),
        "Fix: integer ballot predicate must be coerced to bool via `!= 0u`.\n{wgsl}",
    );
}

#[test]
fn async_nodes_are_rejected_in_naga_emit() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 1, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::async_load("stream-a"),
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::async_wait("stream-a"),
        ],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: async nodes should fail hard in wgpu lowering.");
    let message = err.to_string();
    assert!(
        message.contains("AsyncLoad") && message.contains("Fix:"),
        "Fix: async node rejection must be actionable and not silent.\n{message}",
    );
}

#[test]
fn trap_nodes_lower_to_backend_sidecar_and_return_lane() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 1, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::trap(Expr::u32(0), "trap-tag"),
            Node::store("out", Expr::u32(0), Expr::u32(1)),
        ],
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("vyre_wgpu_trap_sidecar")
            && wgsl.contains("atomicCompareExchangeWeak")
            && wgsl.contains("return;"),
        "Fix: Node::Trap must lower to a backend-owned atomic sidecar and terminate the lane.\n{wgsl}",
    );
}

#[test]
fn resume_nodes_are_rejected_in_naga_emit() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 1, DataType::U32)],
        [1, 1, 1],
        vec![Node::resume("trap-tag")],
    );

    let err = emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect_err("Fix: resume nodes should fail hard in wgpu lowering.");
    let message = err.to_string();
    assert!(
        message.contains("Resume") && message.contains("Fix:"),
        "Fix: resume rejection must be actionable and not silent.\n{message}",
    );
}
