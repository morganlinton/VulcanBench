//! Stress tests for the fusion pass on buffer-write-heavy programs.
//!
//! Background: `audits/VYRE_OPTIMIZER.md` documents an O(n²) hazard when
//! frequent `flush_for_buffer` calls interact with a large pending-replacement
//! set. `FactSubstrate` fixed use-count recomputation, but the replacement
//! flush path may still be quadratic. These tests exercise the pass on shapes
//! that historically triggered the slowdown.

use std::time::{Duration, Instant};

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::fusion::Fusion;
use vyre_foundation::optimizer::{PassScheduler, ProgramPassKind};
use vyre_reference::value::Value;

/// Unwrap the synthetic root region that `Program::wrapped` injects when the
/// entry contains non-Region nodes.
fn entry_body(program: &Program) -> &[Node] {
    match program.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => entry,
    }
}

/// Run **only** the fusion pass.
fn run_fusion(program: Program) -> Program {
    PassScheduler::with_passes(vec![ProgramPassKind::new(Fusion)])
        .run(program)
        .expect("fusion pass must converge")
}

// ---------------------------------------------------------------------------
// 1. Many sequential buffer writes
// ---------------------------------------------------------------------------

#[test]
fn many_sequential_buffer_writes_completes_fast() {
    const N: usize = 150;

    let buffers: Vec<BufferDecl> = (0..N)
        .map(|i| BufferDecl::read_write(&format!("buf_{i}"), i as u32, DataType::U32).with_count(1))
        .collect();

    let mut body = Vec::with_capacity(N * 2 + 1);
    for i in 0..N {
        body.push(Node::let_bind(
            format!("x_{i}"),
            Expr::add(Expr::u32(i as u32), Expr::u32(1)),
        ));
    }
    for i in 0..N {
        body.push(Node::store(
            format!("buf_{i}"),
            Expr::u32(0),
            Expr::var(format!("x_{i}")),
        ));
    }
    body.push(Node::Return);

    let program = Program::wrapped(buffers.clone(), [1, 1, 1], body);

    let start = Instant::now();
    let optimized = run_fusion(program.clone());
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "fusion took {elapsed:?}, expected < 2 s for {N} buffer writes"
    );

    // All single-use pure lets should have been inlined into their consumer.
    let body = entry_body(&optimized);
    let let_count = body
        .iter()
        .filter(|n| matches!(n, Node::Let { .. }))
        .count();
    assert_eq!(let_count, 0, "all pure single-use lets should be inlined");

    // Semantic equivalence via the reference interpreter.
    let inputs: Vec<Value> = (0..N).map(|_| Value::U32(0)).collect();
    let original_out =
        vyre_reference::reference_eval(&program, &inputs).expect("original must run");
    let optimized_out =
        vyre_reference::reference_eval(&optimized, &inputs).expect("optimized must run");
    assert_eq!(
        original_out, optimized_out,
        "fusion must preserve semantics"
    );
}

// ---------------------------------------------------------------------------
// 2. Alternating read/write to the same buffer
// ---------------------------------------------------------------------------

#[test]
fn alternating_read_write_flushes_replacements_before_store() {
    const N: usize = 50;

    let buffers = vec![BufferDecl::read_write("A", 0, DataType::U32).with_count(1)];

    let mut body = Vec::with_capacity(N * 4 + 1);
    for i in 0..N {
        body.push(Node::let_bind(
            format!("x_{i}"),
            Expr::load("A", Expr::u32(0)),
        ));
        body.push(Node::store(
            "A",
            Expr::u32(0),
            Expr::add(Expr::var(format!("x_{i}")), Expr::u32(1)),
        ));
        body.push(Node::let_bind(
            format!("y_{i}"),
            Expr::load("A", Expr::u32(0)),
        ));
        body.push(Node::store(
            "A",
            Expr::u32(0),
            Expr::mul(Expr::var(format!("y_{i}")), Expr::u32(2)),
        ));
    }
    body.push(Node::Return);

    let program = Program::wrapped(buffers, [1, 1, 1], body);
    let optimized = run_fusion(program.clone());

    let body = entry_body(&optimized);

    // Every load depends on buffer A, so each Store(A, ...) must flush the
    // pending replacement, preventing it from being fused into the store.
    let let_count = body
        .iter()
        .filter(|n| matches!(n, Node::Let { .. }))
        .count();
    assert_eq!(
        let_count,
        N * 2,
        "each load binding must survive fusion because it depends on the buffer being written"
    );

    let inputs = [Value::U32(1)];
    let original_out =
        vyre_reference::reference_eval(&program, &inputs).expect("original must run");
    let optimized_out =
        vyre_reference::reference_eval(&optimized, &inputs).expect("optimized must run");
    assert_eq!(
        original_out, optimized_out,
        "fusion must preserve semantics"
    );
}

// ---------------------------------------------------------------------------
// 3. Single-use chain
// ---------------------------------------------------------------------------

#[test]
fn single_use_chain_inlines_without_exponential_blowup() {
    const N: usize = 100;

    let buffers = vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)];

    let mut body = Vec::with_capacity(N + 2);
    body.push(Node::let_bind("a_0", Expr::u32(1)));
    for i in 1..N {
        body.push(Node::let_bind(
            format!("a_{i}"),
            Expr::add(Expr::var(format!("a_{}", i - 1)), Expr::u32(1)),
        ));
    }
    body.push(Node::store(
        "out",
        Expr::u32(0),
        Expr::var(format!("a_{}", N - 1)),
    ));
    body.push(Node::Return);

    let program = Program::wrapped(buffers, [1, 1, 1], body);

    let start = Instant::now();
    let optimized = run_fusion(program.clone());
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "fusion took {elapsed:?}, expected < 2 s for chain of {N} bindings"
    );

    let body = entry_body(&optimized);
    // a_0 is a literal (trivial, not fusable), everything else is inlined.
    assert!(
        body.len() <= 3,
        "expected at most Let a_0, Store, and Return, got {body:?}"
    );

    let inputs = [Value::U32(0)];
    let original_out =
        vyre_reference::reference_eval(&program, &inputs).expect("original must run");
    let optimized_out =
        vyre_reference::reference_eval(&optimized, &inputs).expect("optimized must run");
    assert_eq!(
        original_out, optimized_out,
        "fusion must preserve semantics"
    );
}

// ---------------------------------------------------------------------------
// 4. Deeply nested Select in store
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_select_in_store_no_stack_overflow() {
    const DEPTH: usize = 25;

    let buffers = vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)];

    let mut body = Vec::with_capacity(DEPTH + 2);
    body.push(Node::let_bind("s_0", Expr::u32(42)));
    for i in 1..DEPTH {
        body.push(Node::let_bind(
            format!("s_{i}"),
            Expr::select(
                Expr::u32(1),
                Expr::var(format!("s_{}", i - 1)),
                Expr::u32(0),
            ),
        ));
    }
    body.push(Node::store(
        "out",
        Expr::u32(0),
        Expr::var(format!("s_{}", DEPTH - 1)),
    ));
    body.push(Node::Return);

    let program = Program::wrapped(buffers, [1, 1, 1], body);

    let start = Instant::now();
    let optimized = run_fusion(program.clone());
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "fusion took {elapsed:?}, expected < 2 s for select depth {DEPTH}"
    );

    let inputs = [Value::U32(0)];
    let original_out =
        vyre_reference::reference_eval(&program, &inputs).expect("original must run");
    let optimized_out =
        vyre_reference::reference_eval(&optimized, &inputs).expect("optimized must run");
    assert_eq!(
        original_out, optimized_out,
        "fusion must preserve semantics"
    );
}

// ---------------------------------------------------------------------------
// 5. Many independent pure bindings
// ---------------------------------------------------------------------------

#[test]
fn many_independent_pure_bindings_inlined_without_duplication() {
    const N: usize = 200;

    let buffers = vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(N as u32)];

    let mut body = Vec::with_capacity(N * 2 + 1);
    for i in 0..N {
        body.push(Node::let_bind(
            format!("x_{i}"),
            Expr::add(Expr::u32(i as u32), Expr::u32(1)),
        ));
    }
    for i in 0..N {
        body.push(Node::store(
            "out",
            Expr::u32(i as u32),
            Expr::var(format!("x_{i}")),
        ));
    }
    body.push(Node::Return);

    let program = Program::wrapped(buffers.clone(), [1, 1, 1], body);
    let optimized = run_fusion(program.clone());

    let orig_body = entry_body(&program);
    let opt_body = entry_body(&optimized);

    // All single-use pure lets should disappear.
    let let_count = opt_body
        .iter()
        .filter(|n| matches!(n, Node::Let { .. }))
        .count();
    assert_eq!(
        let_count, 0,
        "all {N} single-use pure lets should be inlined"
    );

    // Because each binding is used exactly once, inlining does not duplicate
    // code. The optimized entry length (stores only) should be proportional
    // to the original, not exponentially larger.
    assert!(
        opt_body.len() * 2 >= orig_body.len(),
        "optimized entry length {} should be close to original {} (no duplication blowup)",
        opt_body.len(),
        orig_body.len()
    );

    // Wire size should stay bounded as well.
    let orig_wire = program.to_wire().expect("original must encode");
    let opt_wire = optimized.to_wire().expect("optimized must encode");
    assert!(
        opt_wire.len() <= orig_wire.len() * 2,
        "optimized wire size {} should not be much larger than original {} (no duplication)",
        opt_wire.len(),
        orig_wire.len()
    );

    let inputs = [Value::Array(vec![Value::U32(0); N])];
    let original_out =
        vyre_reference::reference_eval(&program, &inputs).expect("original must run");
    let optimized_out =
        vyre_reference::reference_eval(&optimized, &inputs).expect("optimized must run");
    assert_eq!(
        original_out, optimized_out,
        "fusion must preserve semantics"
    );
}
