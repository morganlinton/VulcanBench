use super::*;

#[test]
fn ptx_emits_select() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::select(
                Expr::eq(Expr::gid_x(), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(2),
            ),
        )],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: Select must lower to PTX.");
    assert!(
        secondary_text.contains("selp.u32"),
        "Fix: Select must emit selp instruction."
    );
}

#[test]
fn ptx_emits_barrier() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [64, 1, 1],
        vec![
            Node::Barrier {
                ordering: vyre::memory_model::MemoryOrdering::SeqCst,
            },
            Node::store("out", Expr::gid_x(), Expr::u32(0)),
        ],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: Barrier must lower to PTX.");
    assert!(
        secondary_text.contains("bar.sync 0"),
        "Fix: Barrier must emit bar.sync."
    );
}

// ── Invocation IDs ───────────────────────────────────────────────────

#[test]
fn ptx_emits_all_three_axis_ids() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(3)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::gid_x()),
            Node::store("out", Expr::u32(1), Expr::InvocationId { axis: 1 }),
            Node::store("out", Expr::u32(2), Expr::InvocationId { axis: 2 }),
        ],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: all 3 axis IDs must lower to PTX.");
    assert!(
        secondary_text.contains("%r3"),
        "Fix: axis-0 gid must use %r3."
    );
    assert!(
        secondary_text.contains("%r7"),
        "Fix: axis-1 gid must use %r7."
    );
    assert!(
        secondary_text.contains("%r25"),
        "Fix: axis-2 gid must use %r25."
    );
}

// ── Atomics ──────────────────────────────────────────────────────────

#[test]
fn ptx_emits_atomic_add() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Let {
            name: "old".into(),
            value: Expr::atomic_add("buf", Expr::u32(0), Expr::u32(1)),
        }],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: atomic add must lower to PTX.");
    assert!(
        secondary_text.contains("atom.global.add.u32"),
        "Fix: atomic add must emit atom.global.add.u32."
    );
}

// ── Cast ─────────────────────────────────────────────────────────────

#[test]
fn ptx_emits_u32_to_f32_cast() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::cast(DataType::F32, Expr::gid_x()),
        )],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: u32→f32 cast must lower to PTX.");
    assert!(
        secondary_text.contains("cvt.rn.f32.u32"),
        "Fix: u32→f32 cast must emit cvt.rn.f32.u32."
    );
}

// ── Shared memory ────────────────────────────────────────────────────

#[test]
fn ptx_declares_shared_memory() {
    let program = Program::wrapped(
        vec![
            BufferDecl::workgroup("scratch", 16, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [64, 1, 1],
        vec![
            Node::store("scratch", Expr::u32(0), Expr::u32(7)),
            Node::Barrier {
                ordering: vyre::memory_model::MemoryOrdering::SeqCst,
            },
            Node::store("out", Expr::u32(0), Expr::load("scratch", Expr::u32(0))),
        ],
    );
    let secondary_text =
        program_to_ptx(&program, &default_config()).expect("Fix: shared memory must lower to PTX.");
    assert!(
        secondary_text.contains(".shared .align 4"),
        "Fix: PTX must declare .shared memory for workgroup buffers."
    );
}

// ── Subgroup ops are lowered by ptx_emits_integer_subgroup_ops ───────

#[test]
fn ptx_emits_subgroup_ballot() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::SubgroupBallot {
                cond: Box::new(Expr::bool(true)),
            },
        )],
    );
    let secondary_text = program_to_ptx(&program, &default_config())
        .expect("Fix: subgroup ballot must lower to CUDA warp vote PTX.");
    assert!(
        secondary_text.contains("activemask.b32")
            && secondary_text.contains("vote.sync.ballot.b32"),
        "Fix: subgroup ballot must use active-mask guarded CUDA warp vote."
    );
}

// ── PTX must be 7-bit ASCII ──────────────────────────────────────────
// ptxas rejects any non-ASCII byte even in comments with the cryptic
// `Unexpected non-ASCII character encountered on line N` fatal. A single
// em-dash, en-dash, smart-quote, or U+2248 (≈) anywhere in an emitted
// .ptx string takes down every kernel that path produces. Guard every
// barrier ordering and a hand-picked mix of ops that exercise the comment
// emitters and operator labels.

fn assert_ptx_is_ascii(label: &str, ptx: &str) {
    if let Some((idx, byte)) = ptx
        .as_bytes()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, b)| *b > 0x7f)
    {
        let line_no = ptx[..idx].bytes().filter(|b| *b == b'\n').count() + 1;
        let line = ptx.lines().nth(line_no - 1).unwrap_or("");
        panic!(
            "Fix: {label} emitted non-ASCII byte 0x{byte:02x} at offset {idx} (line {line_no}). \
             ptxas rejects non-ASCII even inside comments. Replace em-dashes, smart-quotes, \
             and U+22xx math glyphs in PTX comment emitters with ASCII equivalents. \
             Offending line: {line:?}"
        );
    }
}

#[test]
fn ptx_is_pure_ascii_for_every_barrier_ordering() {
    use vyre::memory_model::MemoryOrdering;
    for ordering in [
        MemoryOrdering::Acquire,
        MemoryOrdering::Release,
        MemoryOrdering::AcqRel,
        MemoryOrdering::SeqCst,
    ] {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [64, 1, 1],
            vec![
                Node::Barrier { ordering },
                Node::store("out", Expr::gid_x(), Expr::u32(0)),
            ],
        );
        let ptx = program_to_ptx(&program, &default_config())
            .unwrap_or_else(|e| panic!("Fix: barrier {ordering:?} must lower to PTX: {e}"));
        assert_ptx_is_ascii(&format!("Barrier({ordering:?})"), &ptx);
    }
}

#[test]
fn ptx_emits_cooperative_grid_barrier_not_cta_downgrade() {
    use vyre::memory_model::MemoryOrdering;
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [64, 1, 1],
        vec![
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            },
            Node::store("out", Expr::gid_x(), Expr::u32(0)),
        ],
    );

    // CUDA lowers GridSync to a NATIVE cooperative grid barrier (it launches such
    // programs cooperatively). The PTX must contain the whole-grid machinery, NOT
    // a silent downgrade to a CTA-scope `bar.sync 0`: a module-scope arrival
    // counter, a global atomic on it, and global memory fences.
    let ptx = program_to_ptx(&program, &default_config())
        .unwrap_or_else(|e| panic!("Fix: CUDA must lower GridSync to a cooperative grid barrier: {e}"));
    assert_ptx_is_ascii("GridSync", &ptx);
    assert!(
        ptx.contains(".global .align 4 .u32 _vyre_grid_barrier[1];"),
        "Fix: GridSync PTX must declare the module-scope cooperative barrier counter; got:\n{ptx}"
    );
    assert!(
        ptx.contains("atom.global.add.u32") && ptx.contains("membar.gl"),
        "Fix: GridSync PTX must use a global arrival atomic + global memory fence (grid scope), not a bare CTA bar.sync; got:\n{ptx}"
    );
    let cta_only_barriers = ptx.matches("bar.sync 0;").count();
    assert!(
        cta_only_barriers >= 2,
        "Fix: the cooperative grid barrier brackets the global phase with two CTA bar.sync converge points; got {cta_only_barriers} in:\n{ptx}"
    );
}

#[test]
fn ptx_is_pure_ascii_for_identity_program() {
    let ptx = program_to_ptx(&identity_program(), &default_config())
        .expect("Fix: identity program must lower to PTX.");
    assert_ptx_is_ascii("identity_program", &ptx);
}

// ── Unsupported nodes ────────────────────────────────────────────────

#[test]
fn ptx_rejects_indirect_dispatch() {
    let program = Program::wrapped(
        vec![BufferDecl::read("buf", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::IndirectDispatch {
            count_buffer: "buf".into(),
            count_offset: 0,
        }],
    );
    let err = program_to_ptx(&program, &default_config())
        .expect_err("Fix: IndirectDispatch must be rejected.");
    assert!(
        err.contains("IndirectDispatch"),
        "Fix: error must name IndirectDispatch, got: {err}"
    );
}
