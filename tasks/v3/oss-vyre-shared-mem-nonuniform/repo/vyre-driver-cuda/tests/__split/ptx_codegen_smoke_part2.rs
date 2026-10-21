use super::*;

#[test]
fn ptx_emits_bitwise_ops() {
    let ops = [
        (
            "and",
            Expr::bitand(Expr::gid_x(), Expr::u32(0xFF)),
            "and.b32",
        ),
        ("or", Expr::bitor(Expr::gid_x(), Expr::u32(0xFF)), "or.b32"),
        (
            "xor",
            Expr::bitxor(Expr::gid_x(), Expr::u32(0xFF)),
            "xor.b32",
        ),
        ("shl", Expr::shl(Expr::gid_x(), Expr::u32(2)), "shl.b32"),
        ("shr", Expr::shr(Expr::gid_x(), Expr::u32(2)), "shr.u32"),
    ];
    for (name, expr, expected_insn) in ops {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|e| panic!("Fix: {name} must lower to PTX: {e}"));
        assert!(
            secondary_text.contains(expected_insn),
            "Fix: {name} must emit {expected_insn}."
        );
    }
}

#[test]
fn grouped_int4_affine_ptx_masks_power_of_two_modulo() {
    let spec = vyre_libs::nn::QuantizedLinear4BitSpec::affine_grouped(256, 4096, 64);
    let program =
        vyre_libs::nn::linear_4bit_affine_grouped_typed(&spec, "x", "w", "scale", "zp", "b", "out")
            .expect("Fix: grouped INT4 affine release program must build.");
    let ptx = program_to_ptx_for_sm(&program, &default_config(), 90)
        .expect("Fix: grouped INT4 affine release program must lower to PTX.");

    assert!(
        ptx.contains("and.b32"),
        "Fix: grouped INT4 PTX must use masks for power-of-two modulo in the hot path.\n{ptx}"
    );
    assert!(
        !ptx.contains("rem.u32"),
        "Fix: grouped INT4 PTX must not emit slow total u32 modulo in the hot path.\n{ptx}"
    );
}

#[test]
fn grouped_int4_affine_ptx_broadcasts_packed_weight_words() {
    let spec = vyre_libs::nn::QuantizedLinear4BitSpec::affine_grouped(256, 4096, 64);
    let program =
        vyre_libs::nn::linear_4bit_affine_grouped_typed(&spec, "x", "w", "scale", "zp", "b", "out")
            .expect("Fix: grouped INT4 affine release program must build.");
    let ptx = program_to_ptx_for_sm(&program, &default_config(), 90)
        .expect("Fix: grouped INT4 affine release program must lower to PTX.");

    assert!(
        ptx.contains("shfl.sync.idx.b32"),
        "Fix: grouped INT4 PTX must broadcast each packed weight word from its 8-lane leader.\n{ptx}"
    );
    assert!(
        ptx.matches("shfl.sync.idx.b32").count() >= 3,
        "Fix: grouped INT4 PTX must broadcast packed weight, scale, and zero-point values instead of reloading sidecars per lane.\n{ptx}"
    );
}

#[test]
fn ptx_emits_integer_comparisons() {
    // The `cmp_normalize` lowering pass (vyre-lower) canonicalizes the
    // inverse-relation comparisons for CSE: `Gt(a, b)` -> `Lt(b, a)` and
    // `Ge(a, b)` -> `Le(b, a)`. Backend emitters produce equivalent code
    // for Lt/Gt and Le/Ge, so the canonical direction is free at codegen.
    // Therefore a `gt` Expr reaches PTX as `lt` (operands swapped) and
    // emits `setp.lt.u32`; a `ge` Expr emits `setp.le.u32`. Asserting the
    // normalized instruction still proves the chain fired -- an
    // un-normalized `Gt` would emit `setp.gt.u32` and fail this check.
    let ops = [
        (
            "eq",
            Expr::eq(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
            "setp.eq.u32",
        ),
        (
            "ne",
            Expr::ne(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
            "setp.ne.u32",
        ),
        (
            "lt",
            Expr::lt(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
            "setp.lt.u32",
        ),
        (
            // Gt(input, 5) -> Lt(5, input) via cmp_normalize.
            "gt",
            Expr::gt(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
            "setp.lt.u32",
        ),
        (
            "le",
            Expr::le(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
            "setp.le.u32",
        ),
        (
            // Ge(input, 5) -> Le(5, input) via cmp_normalize.
            "ge",
            Expr::ge(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
            "setp.le.u32",
        ),
    ];
    for (name, expr, expected_insn) in ops {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(1),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|e| panic!("Fix: {name} must lower to PTX: {e}"));
        assert!(
            secondary_text.contains(expected_insn),
            "Fix: {name} must emit {expected_insn}."
        );
    }
}

#[test]
fn ptx_emits_float_arithmetic() {
    let ops = [
        (
            "fadd",
            Expr::add(Expr::load("input", Expr::u32(0)), Expr::f32(2.0)),
            "add.f32",
        ),
        (
            "fsub",
            Expr::sub(Expr::load("input", Expr::u32(0)), Expr::f32(2.0)),
            "sub.f32",
        ),
        (
            "fmul",
            Expr::mul(Expr::load("input", Expr::u32(0)), Expr::f32(3.0)),
            "mul.f32",
        ),
        (
            "fdiv",
            Expr::div(Expr::load("input", Expr::u32(0)), Expr::f32(3.0)),
            "mul.f32",
        ),
    ];
    for (name, expr, expected_insn) in ops {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::F32).with_count(1),
                BufferDecl::output("out", 1, DataType::F32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|e| panic!("Fix: {name} must lower to PTX: {e}"));
        assert!(
            secondary_text.contains(expected_insn),
            "Fix: {name} must emit {expected_insn}."
        );
    }
}

// ── UnOp coverage ────────────────────────────────────────────────────

#[test]
fn ptx_emits_unary_ops() {
    let ops: Vec<(&str, Expr, &str, DataType, DataType)> = vec![
        (
            "neg_f32",
            Expr::negate(Expr::load("input", Expr::u32(0))),
            "neg.f32",
            DataType::F32,
            DataType::F32,
        ),
        (
            "not_b32",
            Expr::bitnot(Expr::load("input", Expr::u32(0))),
            "not.b32",
            DataType::U32,
            DataType::U32,
        ),
        (
            "abs_f32",
            Expr::abs(Expr::load("input", Expr::u32(0))),
            "abs.f32",
            DataType::F32,
            DataType::F32,
        ),
        (
            "sqrt_f32",
            Expr::sqrt(Expr::load("input", Expr::u32(0))),
            "sqrt.rn.f32",
            DataType::F32,
            DataType::F32,
        ),
        (
            "popc",
            Expr::popcount(Expr::load("input", Expr::u32(0))),
            "popc.b32",
            DataType::U32,
            DataType::U32,
        ),
        (
            "clz",
            Expr::clz(Expr::load("input", Expr::u32(0))),
            "clz.b32",
            DataType::U32,
            DataType::U32,
        ),
        (
            "is_inf",
            Expr::is_inf(Expr::load("input", Expr::u32(0))),
            "setp.eq.u32",
            DataType::F32,
            DataType::Bool,
        ),
        (
            "is_finite",
            Expr::is_finite(Expr::load("input", Expr::u32(0))),
            "setp.lt.u32",
            DataType::F32,
            DataType::Bool,
        ),
    ];
    for (name, expr, expected_insn, input_dt, output_dt) in ops {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, input_dt).with_count(1),
                BufferDecl::output("out", 1, output_dt).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|e| panic!("Fix: {name} must lower to PTX: {e}"));
        assert!(
            secondary_text.contains(expected_insn),
            "Fix: {name} must emit {expected_insn}."
        );
    }
}

#[test]
fn ptx_uses_strict_inverse_sqrt_without_ulp_budget() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(1),
            BufferDecl::output("out", 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::inverse_sqrt(Expr::load("input", Expr::u32(0))),
        )],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: strict inverse-sqrt must lower without approximate PTX.");
    assert!(
        secondary_text.contains("sqrt.rn.f32")
            && secondary_text.contains("rcp.rn.f32")
            && !secondary_text.contains("rsqrt.approx.f32"),
        "Fix: strict inverse-sqrt must avoid rsqrt.approx without an explicit ULP budget; got:\n{secondary_text}"
    );
}

#[test]
fn ptx_requires_ulp_budget_for_approximate_transcendentals() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(1),
            BufferDecl::output("out", 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::UnOp {
                op: UnOp::Tanh,
                operand: Box::new(Expr::load("input", Expr::u32(0))),
            },
        )],
    );
    let err = program_to_ptx(&program, &default_config())
        .expect_err("Fix: CUDA tanh must reject implicit approximate lowering.");
    assert!(
        err.contains("tanh") && err.contains("ulp_budget") && err.contains("Fix:"),
        "Fix: approximate-transcendental rejection must name the op and remediation; got: {err}"
    );

    let mut config = default_config();
    config.ulp_budget = Some(64);
    let secondary_text = program_to_ptx(&program, &config)
        .expect("Fix: explicit ULP budget must permit approximate tanh PTX.");
    assert!(
        secondary_text.contains("tanh.approx.f32"),
        "Fix: budgeted tanh lowering must use the PTX fast approximation; got:\n{secondary_text}"
    );
}

#[test]
fn ptx_emits_integer_subgroup_ops() {
    let ops: Vec<(&str, Expr, &str)> = vec![
        (
            "subgroup_ballot",
            Expr::SubgroupBallot {
                cond: Box::new(Expr::eq(Expr::gid_x(), Expr::u32(0))),
            },
            "vote.sync.ballot.b32",
        ),
        (
            "subgroup_shuffle",
            Expr::SubgroupShuffle {
                value: Box::new(Expr::gid_x()),
                lane: Box::new(Expr::u32(0)),
            },
            "shfl.sync.idx.b32",
        ),
        (
            "subgroup_add",
            Expr::subgroup_add(Expr::gid_x()),
            "redux.sync.add.u32",
        ),
    ];
    for (name, expr, expected_insn) in ops {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|e| panic!("Fix: {name} must lower to PTX: {e}"));
        assert!(
            secondary_text.contains("activemask.b32") && secondary_text.contains(expected_insn),
            "Fix: {name} must emit active-mask guarded {expected_insn}."
        );
    }
}

#[test]
fn ptx_lowers_workgroup_sum_region_to_subgroup_reduction() {
    let program = Program::wrapped(
        vec![
            BufferDecl::workgroup("scratch", 256, DataType::F32),
            BufferDecl::output("out", 0, DataType::F32).with_count(256),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("local", Expr::LocalId { axis: 0 }),
            Node::store("scratch", Expr::var("local"), Expr::f32(1.0)),
            Node::Region {
                generator: "vyre-primitives::reduce::workgroup_sum_f32_child".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![
                    Node::store(
                        "scratch",
                        Expr::var("local"),
                        Expr::load("scratch", Expr::var("local")),
                    ),
                    Node::barrier(),
                ]),
            },
            Node::store(
                "out",
                Expr::var("local"),
                Expr::load("scratch", Expr::var("local")),
            ),
        ],
    );

    let secondary_text = program_to_ptx_for_sm_and_subgroup(&program, &default_config(), 120, 32)
        .expect("Fix: CUDA codegen must lower canonical workgroup sum regions to PTX.");
    // f32 has no redux.sync support, so the canonical workgroup-sum lowering
    // reduces with a shfl XOR butterfly: shfl.sync.idx.b32 with an explicit
    // `laneid ^ offset` source, combined by add.f32. Two instructions must NOT
    // appear: redux.sync.add.f32 (invalid PTX for f32), and shfl.sync.down.b32
    // (a down-tree feeds only lane 0, violating the all-lane-broadcast contract
    // that subgroup_reduce_gpu_parity verifies on a live sm_120 GPU).
    assert!(
        secondary_text.contains("shfl.sync.idx.b32")
            && secondary_text.contains("add.f32")
            && !secondary_text.contains("redux.sync.add.f32")
            && !secondary_text.contains("shfl.sync.down.b32"),
        "Fix: CUDA codegen must invoke f32-safe XOR-butterfly subgroup lowering (shfl.sync.idx.b32 + add.f32, no redux.sync.add.f32, no shfl.sync.down.b32) before PTX emission for workgroup-tree reductions, got:\n{secondary_text}"
    );
}

// ── FMA ──────────────────────────────────────────────────────────────

#[test]
fn ptx_emits_fma() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::F32).with_count(1),
            BufferDecl::read("b", 1, DataType::F32).with_count(1),
            BufferDecl::read("c", 2, DataType::F32).with_count(1),
            BufferDecl::output("out", 3, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::fma(
                Expr::load("a", Expr::u32(0)),
                Expr::load("b", Expr::u32(0)),
                Expr::load("c", Expr::u32(0)),
            ),
        )],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: FMA must lower to PTX.");
    assert!(
        secondary_text.contains("fma.rn.f32"),
        "Fix: FMA must emit fma.rn.f32 instruction."
    );
}

// ── Control flow ─────────────────────────────────────────────────────

#[test]
fn ptx_emits_if_then_else() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::If {
            cond: Expr::eq(Expr::gid_x(), Expr::u32(0)),
            then: vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
            otherwise: vec![Node::store("out", Expr::u32(0), Expr::u32(2))],
        }],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: If/else must lower to PTX.");
    assert!(
        secondary_text.contains("@%") && secondary_text.contains("@!%"),
        "Fix: simple PTX if/else store bodies must lower to complementary predicated stores."
    );
    assert!(
        secondary_text.matches("st.global.u32").count() == 2,
        "Fix: PTX if/else must emit both predicated stores."
    );
    assert!(
        !secondary_text.contains("$L_if_else_") && !secondary_text.contains("$L_if_end_"),
        "Fix: branchless predication must not leave if/else labels in simple store bodies."
    );
}

#[test]
fn ptx_prunes_literal_false_if_branch_before_lowering_dead_code() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::If {
            cond: Expr::bool(false),
            then: vec![Node::let_bind(
                "dead_call",
                Expr::Call {
                    op_id: "unknown.op".into(),
                    args: Vec::new(),
                },
            )],
            otherwise: vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        }],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: literal-false If must prune unsupported dead branch before PTX lowering.");
    assert!(
        secondary_text.contains("st.global.u32") && !secondary_text.contains("dead_call"),
        "Fix: literal-false If lowering must emit only the reachable branch."
    );
}

#[test]
fn ptx_emits_trap_as_lane_exit() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Trap {
            address: Box::new(Expr::u32(7)),
            tag: "decode.invalid".into(),
        }],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: Trap must lower to PTX.");
    assert!(
        secondary_text.contains("// trap tag: decode.invalid")
            && secondary_text.contains("bra $L_exit;"),
        "Fix: CUDA trap lowering must preserve the tag in PTX comments and terminate the lane."
    );
    assert!(
        !secondary_text.contains("__vyre_descriptor_trap_sidecar"),
        "Fix: CUDA PTX must not expose vyre-lower's internal trap sidecar in the kernel ABI until CUDA implements trap-sidecar readback."
    );
}
