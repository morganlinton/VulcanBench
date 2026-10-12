#![allow(missing_docs)]

mod support;

pub use support::ir_inner;

use vyre_macros::vyre_ast_registry;

vyre_ast_registry! {
    ContractExpr {
        Literal(u32),
        Binary { left: u32, right: u32 },
    }

    ContractNode {
        Return,
        Store(u32, u32),
    }
}

#[test]
fn ast_registry_supports_multiple_enums_without_name_cross_talk() {
    assert_eq!(
        contractexpr_op_id(&ContractExpr::Literal(7)),
        "vyre.contractexpr.literal"
    );
    assert_eq!(
        contractnode_op_id(&ContractNode::Store(1, 2)),
        "vyre.contractnode.store"
    );

    assert_eq!(ContractExpr::Literal(7), ContractExpr::Literal(7));
    assert_ne!(ContractExpr::Literal(7), ContractExpr::Literal(8));
    assert_eq!(ContractNode::Return, ContractNode::Return);
    assert_ne!(ContractNode::Store(1, 2), ContractNode::Store(2, 1));
}

#[test]
fn ast_registry_generates_decoder_for_each_enum() {
    match generate_contractexpr_gpu_vm_decoder() {
        ir_inner::model::node::Node::If { .. } => {}
        other => panic!("expected ContractExpr decoder cascade, got {other:?}"),
    }
    match generate_contractnode_gpu_vm_decoder() {
        ir_inner::model::node::Node::If { .. } => {}
        other => panic!("expected ContractNode decoder cascade, got {other:?}"),
    }
}
