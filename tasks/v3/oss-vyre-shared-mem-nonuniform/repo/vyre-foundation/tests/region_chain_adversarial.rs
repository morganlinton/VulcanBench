//! Adversarial region-chain tests.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

fn nested_block(depth: usize, leaf: Node) -> Node {
    (0..depth).fold(leaf, |node, index| {
        Node::block(vec![
            Node::let_bind(format!("region_depth_{index}"), Expr::u32(index as u32)),
            node,
        ])
    })
}

#[test]
fn overly_deep_blocks_fail_closed_with_depth_error() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [64, 1, 1],
        vec![nested_block(
            64,
            Node::store("out", Expr::u32(0), Expr::u32(42)),
        )],
    );

    let errors = validate(&program);
    assert!(
        errors.iter().any(|error| error.message().contains("V018")),
        "deep nested block chain must be rejected by the depth guard, got {errors:?}"
    );
}

#[test]
fn nested_loop_chain_preserves_fingerprint_after_wire_roundtrip() {
    let mut body = vec![Node::store("out", Expr::u32(0), Expr::u32(1))];
    for index in 0..16 {
        body = vec![Node::loop_(
            format!("i{index}"),
            Expr::u32(0),
            Expr::u32(2),
            body,
        )];
    }
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        body,
    );

    let encoded = program.to_wire().expect("nested loop program must encode");
    let decoded = Program::from_wire(&encoded).expect("nested loop program must decode");

    assert_eq!(
        decoded.fingerprint(),
        program.fingerprint(),
        "nested loop regions must round-trip without structural drift"
    );
}

#[test]
fn call_region_chains_preserve_argument_order() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [32, 1, 1],
        vec![
            Node::call(
                "plugin.region.chain",
                vec![Expr::u32(1), Expr::u32(2), Expr::u32(3)],
            ),
            Node::store("out", Expr::u32(0), Expr::u32(6)),
        ],
    );

    let encoded = program.to_wire().expect("call region chain must encode");
    let decoded = Program::from_wire(&encoded).expect("call region chain must decode");

    assert_eq!(
        decoded.fingerprint(),
        program.fingerprint(),
        "call region argument order must remain stable"
    );
}
