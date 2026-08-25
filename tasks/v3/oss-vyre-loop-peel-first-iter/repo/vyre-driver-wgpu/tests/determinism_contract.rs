//! Determinism contract.
//!
//! See `contracts/release.md`. Vyre's thesis is "same Program +
//! same inputs → byte-identical outputs." This test proves the
//! contract holds across repeated dispatches on the same backend for
//! every proptest-generated program.
//!
//! The generator intentionally includes unused and conditionally-used buffers
//! so bind-group reflection stays honest across lowered shader variants.

use proptest::prelude::*;
use std::sync::OnceLock;
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, UnOp};

fn backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| WgpuBackend::acquire().expect("Fix: GPU required for determinism test"))
}

fn u32_leaf() -> BoxedStrategy<Expr> {
    prop_oneof![
        any::<u32>().prop_map(Expr::u32),
        Just(Expr::gid_x()),
        Just(Expr::workgroup_x()),
        Just(Expr::local_x()),
        Just(Expr::buf_len("input")),
        Just(Expr::load("input", Expr::gid_x())),
        "[a-z]{1,6}".prop_map(|_| Expr::u32(17)),
    ]
    .boxed()
}

fn u32_expr() -> BoxedStrategy<Expr> {
    u32_leaf()
        .prop_recursive(5, 32, 3, |inner| {
            prop_oneof![
                (inner.clone(), inner.clone()).prop_map(|(a, b)| Expr::BinOp {
                    op: BinOp::Add,
                    left: Box::new(a),
                    right: Box::new(b),
                }),
                (inner.clone(), inner.clone()).prop_map(|(a, b)| Expr::BinOp {
                    op: BinOp::BitXor,
                    left: Box::new(a),
                    right: Box::new(b),
                }),
                inner.clone().prop_map(|a| Expr::UnOp {
                    op: UnOp::ReverseBits,
                    operand: Box::new(a),
                }),
                inner.clone().prop_map(|a| Expr::UnOp {
                    op: UnOp::Popcount,
                    operand: Box::new(a),
                }),
                inner.clone().prop_map(|a| Expr::Cast {
                    target: DataType::U32,
                    value: Box::new(a),
                }),
                (bool_expr_from_u32(inner.clone()), inner.clone(), inner).prop_map(
                    |(cond, yes, no)| Expr::Select {
                        cond: Box::new(cond),
                        true_val: Box::new(yes),
                        false_val: Box::new(no),
                    }
                ),
            ]
        })
        .boxed()
}

fn bool_expr_from_u32(inner: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    prop_oneof![
        any::<bool>().prop_map(Expr::bool),
        (inner.clone(), inner.clone()).prop_map(|(a, b)| Expr::BinOp {
            op: BinOp::Lt,
            left: Box::new(a),
            right: Box::new(b),
        }),
        inner.prop_map(|a| Expr::UnOp {
            op: UnOp::LogicalNot,
            operand: Box::new(a),
        }),
    ]
    .boxed()
}

fn program_strategy() -> impl Strategy<Value = vyre::Program> {
    (1u32..=8, u32_expr(), u32_expr(), u32_expr()).prop_map(|(count, first, second, otherwise)| {
        let idx = Expr::gid_x();
        let in_bounds = Expr::BinOp {
            op: BinOp::Lt,
            left: Box::new(idx.clone()),
            right: Box::new(Expr::buf_len("out")),
        };
        let program = vyre::Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(count),
                BufferDecl::output("out", 1, DataType::U32)
                    .with_count(count)
                    .with_output_byte_range(0..(count as usize * 4)),
            ],
            [1, 1, 1],
            vec![
                Node::let_bind("acc", first),
                Node::assign("acc", second),
                Node::if_then_else(
                    in_bounds,
                    vec![Node::store("out", idx.clone(), Expr::var("acc"))],
                    vec![Node::block(vec![Node::store("out", idx, otherwise)])],
                ),
                Node::return_(),
            ],
        );
        program.mark_structurally_validated();
        program
    })
}

fn inputs_strategy(program: &vyre::Program) -> BoxedStrategy<Vec<Vec<u8>>> {
    let input_lengths: Vec<usize> = program
        .buffers()
        .iter()
        .filter(|buffer| !buffer.is_output())
        .map(|buffer| {
            let element_size = match buffer.element() {
                DataType::U32 | DataType::I32 | DataType::F32 | DataType::Bool => 4,
                other => panic!("Fix: unsupported determinism input type {other:?}"),
            };
            buffer.count() as usize * element_size
        })
        .collect();

    match input_lengths.as_slice() {
        [] => Just(Vec::new()).boxed(),
        [len] => prop::collection::vec(any::<u8>(), *len)
            .prop_map(|input| vec![input])
            .boxed(),
        _ => panic!("Fix: determinism strategy currently emits at most one input buffer"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        // Default 32 cases keeps the GPU workspace gate bounded because every
        // case compiles and dispatches a fresh Program twice. Nightly CI can
        // override with `PROPTEST_CASES=1000` for a fuller sweep.
        cases: 32,
        ..ProptestConfig::default()
    })]

    #[test]
    fn dispatch_is_deterministic(p in program_strategy()) {
        let inputs = inputs_strategy(&p).new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();
        let cfg = DispatchConfig::default();

        let first = backend().dispatch(&p, &inputs, &cfg)
            .expect("first dispatch must succeed");
        let second = backend().dispatch(&p, &inputs, &cfg)
            .expect("second dispatch must succeed");
        prop_assert_eq!(
            first, second,
            "determinism contract: two dispatches of the same Program produced divergent bytes"
        );
    }
}
