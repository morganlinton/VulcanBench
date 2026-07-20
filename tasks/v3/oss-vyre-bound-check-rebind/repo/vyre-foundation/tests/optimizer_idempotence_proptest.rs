//! P0 inventory #35  -  [`vyre_foundation::optimizer::pre_lowering`] should reach a fixed
//! point on the canonical wire form: successive runs must not perturb semantics or
//! serialized bytes.
//!
//! P1 inventory #109  -  wire codec round-trips the shape produced by the optimizer
//! (see also inventory #39 for IR ↔ wire field coverage).

use proptest::prelude::*;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::{
    autotune::Autotune, const_fold::ConstFold, dead_buffer_elim::DeadBufferElim, fusion::Fusion,
    normalize_atomics::NormalizeAtomicsPass, strength_reduce::StrengthReduce,
    vectorization::Vectorization,
};
use vyre_foundation::optimizer::pre_lowering as optimize;
use vyre_foundation::optimizer::{PassScheduler, ProgramPassKind};
use vyre_reference::value::Value;

fn test_output_buffer() -> BufferDecl {
    BufferDecl::read_write("out", 0, DataType::U32).with_count(1)
}

fn program_with_body(body: Vec<Node>) -> Program {
    Program::wrapped(vec![test_output_buffer()], [1, 1, 1], body)
}

fn run_reference(program: &Program) -> Result<Vec<Value>, vyre_foundation::Error> {
    vyre_reference::reference_eval(program, &[Value::U32(0)])
}

fn leaf_expr() -> impl Strategy<Value = Expr> {
    prop_oneof![
        (0_u16..=1024).prop_map(|value| Expr::u32(u32::from(value))),
        Just(Expr::gid_x()),
    ]
}

fn non_zero_literal() -> impl Strategy<Value = Expr> {
    (1_u16..=1024).prop_map(|value| Expr::u32(u32::from(value)))
}

fn shift_amount() -> impl Strategy<Value = Expr> {
    (0_u8..=31).prop_map(|value| Expr::u32(u32::from(value)))
}

/// Bounded pure u32 expression surface (aligned with `adversarial_graph_canonical_laws`).
fn u32_expr() -> impl Strategy<Value = Expr> {
    leaf_expr().prop_recursive(5, 64, 4, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::add(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::mul(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::bitand(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::bitor(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::bitxor(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::sub(left, right)),
            (inner.clone(), non_zero_literal()).prop_map(|(left, right)| Expr::div(left, right)),
            (inner.clone(), shift_amount()).prop_map(|(left, right)| Expr::shl(left, right)),
            (inner.clone(), shift_amount()).prop_map(|(left, right)| Expr::shr(left, right)),
        ]
    })
}

fn program_strategy() -> impl Strategy<Value = Program> {
    prop::collection::vec(u32_expr(), 1..16).prop_map(|exprs| {
        let mut body = Vec::with_capacity(exprs.len() + 1);
        for (index, expr) in exprs.into_iter().enumerate() {
            body.push(Node::let_bind(format!("v{index}"), expr));
        }
        body.push(Node::store(
            "out",
            Expr::u32(0),
            Expr::var(format!("v{}", body.len().saturating_sub(1))),
        ));
        program_with_body(body)
    })
}

fn output_only_store(expr: Expr) -> Program {
    program_with_body(vec![Node::store("out", Expr::u32(0), expr)])
}

fn built_in_pass_names() -> [&'static str; 7] {
    [
        "autotune",
        "const_fold",
        "dead_buffer_elim",
        "fusion",
        "normalize_atomics",
        "strength_reduce",
        "vectorization",
    ]
}

fn scheduler_for_pass(pass_name: &str) -> PassScheduler {
    let passes = match pass_name {
        "autotune" => vec![ProgramPassKind::new(Autotune)],
        "const_fold" => vec![ProgramPassKind::new(ConstFold)],
        "dead_buffer_elim" => vec![
            ProgramPassKind::new(Fusion),
            ProgramPassKind::new(DeadBufferElim),
        ],
        "fusion" => vec![ProgramPassKind::new(Fusion)],
        "normalize_atomics" => vec![ProgramPassKind::new(NormalizeAtomicsPass)],
        "strength_reduce" => vec![
            ProgramPassKind::new(ConstFold),
            ProgramPassKind::new(StrengthReduce),
        ],
        "vectorization" => vec![ProgramPassKind::new(Vectorization)],
        other => panic!("Fix: unhandled built-in optimizer pass `{other}`"),
    };
    PassScheduler::with_passes(passes).with_max_iterations(8)
}

fn pass_contract_corpus() -> Vec<Program> {
    let arithmetic = output_only_store(Expr::add(
        Expr::mul(Expr::u32(3), Expr::u32(4)),
        Expr::mul(Expr::gid_x(), Expr::u32(8)),
    ));

    let dead_buffer = Program::wrapped(
        vec![
            BufferDecl::read_write("dead", 0, DataType::U32).with_count(1),
            test_output_buffer(),
        ],
        [1, 1, 1],
        vec![
            Node::store("dead", Expr::u32(0), Expr::u32(99)),
            Node::store("out", Expr::u32(0), Expr::u32(7)),
        ],
    );

    let fusion_candidate = program_with_body(vec![
        Node::let_bind("a", Expr::add(Expr::gid_x(), Expr::u32(1))),
        Node::let_bind("b", Expr::mul(Expr::var("a"), Expr::u32(2))),
        Node::store("out", Expr::u32(0), Expr::var("b")),
    ]);

    let autotune_candidate = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(256)],
        [1, 1, 1],
        vec![Node::store("out", Expr::gid_x(), Expr::gid_x())],
    );

    let atomic_condition = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::atomic_add("out", Expr::u32(0), Expr::u32(1)),
            vec![Node::store("out", Expr::u32(0), Expr::u32(2))],
        )],
    );

    vec![
        arithmetic,
        dead_buffer,
        fusion_candidate,
        autotune_candidate,
        atomic_condition,
    ]
}

fn canonical_wire(program: &Program) -> Vec<u8> {
    program
        .to_wire()
        .unwrap_or_else(|e| panic!("Fix: optimizer pass output must encode: {e}"))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        .. ProptestConfig::default()
    })]

    #[test]
    fn full_optimize_is_idempotent_on_canonical_wire(program in program_strategy()) {
        let ref_original = run_reference(&program)
            .expect("Fix: generated original program must run on the reference interpreter");
        let once = optimize::optimize(program);
        let wire_once = once
            .to_wire()
            .unwrap_or_else(|e| panic!("Fix: optimize output must encode: {e}"));
        let twice = optimize::optimize(once.clone());
        let wire_twice = twice
            .to_wire()
            .unwrap_or_else(|e| panic!("Fix: second optimize must encode: {e}"));
        prop_assert_eq!(&wire_once, &wire_twice);

        let thrice = optimize::optimize(twice.clone());
        let wire_thrice = thrice
            .to_wire()
            .unwrap_or_else(|e| panic!("Fix: third optimize must encode: {e}"));
        prop_assert_eq!(&wire_twice, &wire_thrice);

        let ref_once = run_reference(&once)
            .expect("Fix: once-optimized program must run on the reference interpreter");
        let ref_twice = run_reference(&twice)
            .expect("Fix: twice-optimized program must run on the reference interpreter");
        let ref_thrice = run_reference(&thrice)
            .expect("Fix: thrice-optimized program must run on the reference interpreter");
        prop_assert_eq!(&ref_original, &ref_once);
        prop_assert_eq!(&ref_once, &ref_twice);
        prop_assert_eq!(&ref_twice, &ref_thrice);
    }
}

#[test]
fn optimize_then_wire_roundtrip_preserves_program_smoke() {
    // Mirrors `optimizer_reference_parity_smoke`  -  enough IR for canonicalize, const fold,
    // CSE/DCE, then full wire round-trip.
    let program = output_only_store(Expr::add(
        Expr::mul(Expr::u32(3), Expr::u32(4)),
        Expr::sub(Expr::u32(10), Expr::u32(2)),
    ));
    let optimized = optimize::optimize(program);
    let bytes = optimized
        .to_wire()
        .expect("Fix: optimized smoke program must encode");
    let back = Program::from_wire(&bytes).expect("Fix: optimized smoke program must decode");
    assert_eq!(back, optimized);
}

#[test]
fn every_builtin_optimizer_pass_converges_and_is_idempotent_on_contract_corpus() {
    let corpus = pass_contract_corpus();

    for pass_name in built_in_pass_names() {
        for (case_index, program) in corpus.iter().enumerate() {
            let once = scheduler_for_pass(pass_name)
                .run(program.clone())
                .unwrap_or_else(|e| {
                    panic!(
                        "Fix: optimizer pass `{pass_name}` must converge on contract corpus case {case_index}: {e}"
                    )
                });
            let twice = scheduler_for_pass(pass_name)
                .run(once.clone())
                .unwrap_or_else(|e| {
                    panic!(
                        "Fix: optimizer pass `{pass_name}` must converge after first run on contract corpus case {case_index}: {e}"
                    )
                });
            let thrice = scheduler_for_pass(pass_name)
                .run(twice.clone())
                .unwrap_or_else(|e| {
                    panic!(
                        "Fix: optimizer pass `{pass_name}` must converge after second run on contract corpus case {case_index}: {e}"
                    )
                });

            assert_eq!(
                canonical_wire(&once),
                canonical_wire(&twice),
                "Fix: optimizer pass `{pass_name}` is not idempotent after convergence on contract corpus case {case_index}"
            );
            assert_eq!(
                canonical_wire(&twice),
                canonical_wire(&thrice),
                "Fix: optimizer pass `{pass_name}` did not hold its fixed point on contract corpus case {case_index}"
            );
        }
    }
}
