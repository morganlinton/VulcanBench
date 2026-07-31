use super::*;

#[test]
fn base64_decode_ptx_compiles_with_ptxas() {
    let program =
        vyre_primitives::decode::base64::base64_decode("input", "table", "output", "len", 8);
    let ptx = program_to_ptx_for_sm(&program, &default_config(), 90)
        .expect("Fix: base64 decode must lower to PTX.");
    let dir = tempfile::tempdir().expect("Fix: create temp dir for ptxas smoke.");
    let ptx_path = dir.path().join("base64.ptx");
    let cubin_path = dir.path().join("base64.cubin");
    std::fs::write(&ptx_path, &ptx).expect("Fix: write base64 PTX for ptxas smoke.");
    let output = std::process::Command::new("ptxas")
        .arg("-arch=sm_90")
        .arg(&ptx_path)
        .arg("-o")
        .arg(&cubin_path)
        .output()
        .expect("Fix: ptxas must be available on the CUDA release path.");
    assert!(
        output.status.success(),
        "Fix: base64 decode PTX must assemble with ptxas.\nstdout:\n{}\nstderr:\n{}\nPTX:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        ptx
    );
}

#[test]
fn inflate_stored_ptx_compiles_with_ptxas() {
    let program = vyre_primitives::decode::inflate::inflate_stored("input", "output", "len", 10);
    let ptx = program_to_ptx_for_sm(&program, &default_config(), 90)
        .expect("Fix: inflate stored must lower to PTX.");
    let dir = tempfile::tempdir().expect("Fix: create temp dir for ptxas smoke.");
    let ptx_path = dir.path().join("inflate.ptx");
    let cubin_path = dir.path().join("inflate.cubin");
    std::fs::write(&ptx_path, &ptx).expect("Fix: write inflate PTX for ptxas smoke.");
    let output = std::process::Command::new("ptxas")
        .arg("-arch=sm_90")
        .arg(&ptx_path)
        .arg("-o")
        .arg(&cubin_path)
        .output()
        .expect("Fix: ptxas must be available on the CUDA release path.");
    assert!(
        output.status.success(),
        "Fix: inflate stored PTX must assemble with ptxas.\nstdout:\n{}\nstderr:\n{}\nPTX:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        ptx
    );
}

// ── Header and structure ─────────────────────────────────────────────

#[test]
fn ptx_contains_version_target_and_entry() {
    let secondary_text = program_to_ptx(&identity_program(), &default_config())
        .expect("Fix: identity program must lower to PTX.");
    assert!(
        secondary_text.contains(".version 8.5"),
        "Fix: PTX must declare version 8.5 (pinned in vyre-emit-ptx/src/emitter.rs)."
    );
    assert!(
        secondary_text.contains(".target sm_"),
        "Fix: PTX must declare target."
    );
    assert!(
        secondary_text.contains(".visible .entry main("),
        "Fix: PTX must declare visible entry point."
    );
    assert!(
        secondary_text.contains("ret;"),
        "Fix: PTX must end with ret."
    );
}

#[test]
fn ptx_for_sm_respects_target() {
    let secondary_text = program_to_ptx_for_sm(&identity_program(), &default_config(), 89)
        .expect("Fix: identity program must lower for sm_89.");
    assert!(
        secondary_text.contains(".target sm_89"),
        "Fix: PTX must target the requested SM."
    );
}

#[test]
fn ptx_for_sm_zero_is_rejected() {
    let err = program_to_ptx_for_sm(&identity_program(), &default_config(), 0)
        .expect_err("Fix: sm_0 must be rejected.");
    assert!(
        err.contains("sm_0"),
        "Fix: error must mention sm_0, got: {err}"
    );
}

#[test]
fn ptx_subgroup_size_uses_probed_width_parameter() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::SubgroupSize)],
    );
    let secondary_text = program_to_ptx_for_sm_and_subgroup(&program, &default_config(), 120, 16)
        .expect("Fix: subgroup-size fixture must lower with explicit probed width.");
    assert!(
        secondary_text.contains("mov.u32") && secondary_text.contains(", 16;"),
        "Fix: PTX SubgroupSize lowering must use the probed CUDA warp width, not a hardcoded constant."
    );
}

#[test]
fn ptx_rejects_invalid_subgroup_width() {
    let err = program_to_ptx_for_sm_and_subgroup(&identity_program(), &default_config(), 120, 0)
        .expect_err("Fix: subgroup size 0 must be rejected.");
    assert!(
        err.contains("subgroup size 0") && err.contains("Fix:"),
        "Fix: invalid subgroup-size diagnostics must be actionable, got: {err}"
    );
}

#[test]
fn ptx_shared_memory_declaration_uses_element_byte_width() {
    let program = Program::wrapped(
        vec![
            BufferDecl::workgroup("scratch", 16, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::store("scratch", Expr::u32(0), Expr::u32(7)),
            Node::Barrier {
                ordering: vyre::memory_model::MemoryOrdering::SeqCst,
            },
            Node::store("out", Expr::u32(0), Expr::load("scratch", Expr::u32(0))),
        ],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: live shared-memory fixture must lower to PTX.");
    assert!(
        secondary_text.contains(".shared .align 4 .b8 shared_buf_")
            && secondary_text.contains("[64];"),
        "Fix: CUDA shared-memory declarations must size by element width; got:\n{secondary_text}"
    );
}

#[test]
fn ptx_dynamic_shared_memory_offsets_use_u32_registers() {
    let program = Program::wrapped(
        vec![
            BufferDecl::workgroup("scratch", 16, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32).with_count(16),
        ],
        [16, 1, 1],
        vec![
            Node::store("scratch", Expr::InvocationId { axis: 0 }, Expr::u32(7)),
            Node::Barrier {
                ordering: vyre::memory_model::MemoryOrdering::SeqCst,
            },
            Node::store(
                "out",
                Expr::InvocationId { axis: 0 },
                Expr::load("scratch", Expr::InvocationId { axis: 0 }),
            ),
        ],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: dynamic shared-memory indexing must lower to PTX.");
    assert!(
        secondary_text.contains("mul.lo.u32")
            && secondary_text.contains("mov.u32")
            && !secondary_text.contains("shared_buf_16+%"),
        "Fix: PTX shared-memory dynamic offsets must be 32-bit address offsets, got:\n{secondary_text}"
    );
}

// ── Literal emission ─────────────────────────────────────────────────

#[test]
fn ptx_emits_u32_literal() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: u32 literal must lower to PTX.");
    assert!(
        secondary_text.contains("mov.u32") && secondary_text.contains("42"),
        "Fix: PTX must contain mov.u32 with literal 42."
    );
}

#[test]
fn ptx_emits_f32_literal_hex() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::f32(1.0))],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: f32 literal must lower to PTX.");
    assert!(
        secondary_text.contains("mov.f32") && secondary_text.contains("0f"),
        "Fix: PTX must emit f32 in hex form (0fXXXXXXXX)."
    );
}

#[test]
fn ptx_emits_bool_literal() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::bool(true))],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: bool literal must lower to PTX.");
    assert!(
        secondary_text.contains("mov.u32") && secondary_text.contains(", 1;"),
        "Fix: PTX must emit true as u32 1."
    );
}

// ── BinOp coverage ───────────────────────────────────────────────────

#[test]
fn ptx_emits_integer_arithmetic() {
    let ops = [
        ("add", Expr::add(Expr::gid_x(), Expr::u32(1)), "add.u32"),
        ("sub", Expr::sub(Expr::gid_x(), Expr::u32(1)), "sub.u32"),
        ("mul", Expr::mul(Expr::gid_x(), Expr::u32(3)), "shl.b32"),
        ("div", Expr::div(Expr::gid_x(), Expr::u32(3)), "mul.hi.u32"),
        ("rem", Expr::rem(Expr::gid_x(), Expr::u32(3)), "rem.u32"),
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
            "Fix: {name} must emit {expected_insn}, got:\n{secondary_text}"
        );
    }
}

#[test]
fn ptx_fuses_integer_multiply_accumulate_for_quantized_accumulators() {
    let product = Expr::mul(
        Expr::load("lhs", Expr::u32(0)),
        Expr::load("rhs", Expr::u32(0)),
    );
    let program = Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::I32).with_count(1),
            BufferDecl::read("rhs", 1, DataType::I32).with_count(1),
            BufferDecl::read("acc", 2, DataType::I32).with_count(1),
            BufferDecl::output("out", 3, DataType::I32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(product, Expr::load("acc", Expr::u32(0))),
        )],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: quantized integer accumulator fixture must lower to PTX.");
    assert!(
        secondary_text.contains("mad.lo.s32"),
        "Fix: quantized integer MACs must fuse to single PTX mad.lo.s32, got:\n{secondary_text}"
    );
    assert!(
        !secondary_text.contains("mul.lo.s32") && !secondary_text.contains("add.s32"),
        "Fix: fused quantized integer MACs must not leave dead scalar mul/add instructions, got:\n{secondary_text}"
    );
}

#[test]
fn ptx_guards_unsigned_div_and_mod_by_zero() {
    let ops = [
        (
            "div",
            Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            "div.u32",
            "0xffffffff",
        ),
        (
            "mod",
            Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            "rem.u32",
            "mov.u32",
        ),
    ];
    for (name, expr, arithmetic, zero_result) in ops {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("a", 0, DataType::U32).with_count(1),
                BufferDecl::read("b", 1, DataType::U32).with_count(1),
                BufferDecl::output("out", 2, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config()).unwrap_or_else(|error| {
            panic!("Fix: unsigned {name} must lower to guarded PTX: {error}")
        });
        assert!(
            secondary_text.contains("setp.eq.u32")
                && secondary_text.contains("bra $L_u32_")
                && secondary_text.contains(arithmetic)
                && secondary_text.contains(zero_result),
            "Fix: unsigned {name} must guard zero divisors before raw {arithmetic}; got:\n{secondary_text}"
        );
    }
}

#[test]
fn ptx_guards_signed_div_and_mod_edge_cases() {
    let ops = [
        (
            "div",
            Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            "div.s32",
            "0x80000000",
        ),
        (
            "mod",
            Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            "rem.s32",
            "and.pred",
        ),
    ];
    for (name, expr, arithmetic, edge_marker) in ops {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("a", 0, DataType::I32).with_count(1),
                BufferDecl::read("b", 1, DataType::I32).with_count(1),
                BufferDecl::output("out", 2, DataType::I32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config()).unwrap_or_else(|error| {
            panic!("Fix: signed {name} must lower to guarded PTX: {error}")
        });
        assert!(
            secondary_text.contains("setp.eq.s32")
                && secondary_text.contains("0xffffffff")
                && secondary_text.contains(edge_marker)
                && secondary_text.contains(arithmetic),
            "Fix: signed {name} must guard zero divisors and i32::MIN/-1 before raw {arithmetic}; got:\n{secondary_text}"
        );
    }
}

#[test]
fn ptx_lowers_f32_division() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::F32).with_count(1),
            BufferDecl::read("b", 1, DataType::F32).with_count(1),
            BufferDecl::output("out", 2, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: f32 division must lower to PTX.");
    assert!(
        secondary_text.contains("div.rn.f32"),
        "Fix: f32 division must emit rounded PTX div, got:\n{secondary_text}"
    );
}

#[test]
fn ptx_lowers_u32_saturating_add_and_sub() {
    let cases = [
        (
            "saturating_add",
            BinOp::SaturatingAdd,
            ["add.u32", "setp.lt.u32", "selp.u32"],
        ),
        (
            "saturating_sub",
            BinOp::SaturatingSub,
            ["sub.u32", "setp.lt.u32", "selp.u32"],
        ),
    ];
    for (name, op, expected) in cases {
        let expr = Expr::BinOp {
            op,
            left: Box::new(Expr::load("a", Expr::u32(0))),
            right: Box::new(Expr::load("b", Expr::u32(0))),
        };
        let program = Program::wrapped(
            vec![
                BufferDecl::read("a", 0, DataType::U32).with_count(1),
                BufferDecl::read("b", 1, DataType::U32).with_count(1),
                BufferDecl::output("out", 2, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let secondary_text = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|error| panic!("Fix: {name} must lower to PTX: {error}"));
        for instruction in expected {
            assert!(
                secondary_text.contains(instruction),
                "Fix: {name} must emit {instruction}, got:\n{secondary_text}"
            );
        }
    }
}
