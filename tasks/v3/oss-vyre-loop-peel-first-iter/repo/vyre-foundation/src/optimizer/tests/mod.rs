use super::optimize;
use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
use proptest::prelude::*;

fn leaf_expr() -> impl Strategy<Value = Expr> {
    prop_oneof![
        any::<u16>().prop_map(|value| Expr::u32(u32::from(value))),
        Just(Expr::gid_x()),
    ]
}

fn expr() -> impl Strategy<Value = Expr> {
    leaf_expr().prop_recursive(4, 32, 3, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::add(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::mul(left, right)),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::bitxor(left, right)),
            inner
                .clone()
                .prop_map(|value| Expr::shl(value, Expr::u32(1))),
            (inner.clone(), inner.clone(), inner).prop_map(|(cond, true_val, false_val)| {
                Expr::select(cond, true_val, false_val)
            }),
        ]
    })
}

fn valid_program() -> impl Strategy<Value = Program> {
    prop::collection::vec(expr(), 1..40).prop_map(|exprs| {
        let mut nodes = Vec::with_capacity(exprs.len() + 1);
        for (index, value) in exprs.into_iter().enumerate() {
            nodes.push(Node::let_bind(format!("v{index}"), value));
        }
        nodes.push(Node::store(
            "out",
            Expr::gid_x(),
            Expr::var(format!("v{}", nodes.len().saturating_sub(1))),
        ));
        Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [64, 1, 1],
            nodes,
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..ProptestConfig::default() })]

    #[test]
    fn prop_optimizer_idempotent_after_alloc_reduction(program in valid_program()) {
        let once = optimize(program).expect("Fix: optimizer should converge on generated valid programs");
        let twice = optimize(once.clone()).expect("Fix: optimizer should converge after first optimization");
        prop_assert_eq!(once, twice);
    }

    /// MOD-008 regression: `fingerprint_program` must be stable across
    /// runs (same input → same hash) and distinct across semantically
    /// different programs (different input → different hash, barring
    /// a blake3 collision which is cryptographically infeasible).
    #[test]
    fn prop_fingerprint_program_deterministic(program in valid_program()) {
        let a = super::fingerprint_program(&program);
        let b = super::fingerprint_program(&program);
        prop_assert_eq!(a, b, "fingerprint must be stable for identical programs");
    }
}

#[test]
fn fingerprint_program_distinguishes_trivially_different_programs() {
    // MOD-008  -  adding a node must change the fingerprint. This is a
    // negative-result guarantee: blake3 collision probability is
    // vanishingly low, so any match here is a real bug (wire-format
    // bug or fingerprint shortcut).
    let base = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let extended = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::store("out", Expr::u32(1), Expr::u32(2)),
        ],
    );
    assert_ne!(
        super::fingerprint_program(&base),
        super::fingerprint_program(&extended),
        "fingerprint must differ when the program body changes"
    );
}
