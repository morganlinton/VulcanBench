//! Freeze tests for backend validation defaults.
//!
//! `default_supported_ops` and `node_op_id` are central to dispatch
//! safety: every Node variant must map to a stable op id, and the
//! default supported set must be a superset of the core operations.

use std::collections::HashSet;

use vyre::backend::validation::{
    default_supported_ops, default_supported_ops_with_trap, node_op_id, validate_program,
};
use vyre::backend::{BackendError, VyreBackend};
use vyre::ir::{CollectiveOp, CommGroup, Expr, Node, OpId, Program};

fn collective_nodes() -> [Node; 4] {
    [
        Node::AllReduce {
            buffer: "a".into(),
            op: CollectiveOp::Sum,
            group: CommGroup::WORLD,
        },
        Node::AllGather {
            input: "input".into(),
            output: "out".into(),
            group: CommGroup::WORLD,
        },
        Node::ReduceScatter {
            input: "input".into(),
            output: "out".into(),
            op: CollectiveOp::Max,
            group: CommGroup(7),
        },
        Node::Broadcast {
            buffer: "a".into(),
            root: 1,
            group: CommGroup(7),
        },
    ]
}

#[test]
fn default_supported_ops_contains_core_nodes() {
    let ops = default_supported_ops();
    assert!(
        ops.contains(node_op_id(&Node::Return)),
        "Return must be in default supported ops"
    );
    assert!(
        ops.contains(node_op_id(&Node::barrier())),
        "Barrier must be in default supported ops"
    );
}

#[test]
fn default_supported_ops_with_trap_contains_trap() {
    let ops = default_supported_ops_with_trap();
    assert!(
        ops.contains(node_op_id(&Node::trap(Expr::u32(0), "test"))),
        "Trap must be in default supported ops with trap"
    );
}

#[test]
fn default_supported_ops_without_trap_does_not_contain_trap() {
    let ops = default_supported_ops();
    assert!(
        !ops.contains(node_op_id(&Node::trap(Expr::u32(0), "test"))),
        "Trap must NOT be in default supported ops without trap"
    );
}

#[test]
fn node_op_id_return_is_stable() {
    assert_eq!(node_op_id(&Node::Return), "vyre.node.return");
}

#[test]
fn node_op_id_barrier_is_stable() {
    assert_eq!(node_op_id(&Node::barrier()), "vyre.node.barrier");
}

#[test]
fn node_op_id_trap_is_stable() {
    assert_eq!(
        node_op_id(&Node::trap(Expr::u32(0), "test")),
        "vyre.node.trap"
    );
}

#[test]
fn node_op_id_store_is_stable() {
    assert_eq!(
        node_op_id(&Node::store("buf", Expr::u32(0), Expr::u32(1))),
        "vyre.node.store"
    );
}

#[test]
fn node_op_id_async_load_is_stable() {
    assert_eq!(node_op_id(&Node::async_load("tag")), "vyre.node.async_load");
}

#[test]
fn node_op_id_async_wait_is_stable() {
    assert_eq!(node_op_id(&Node::async_wait("tag")), "vyre.node.async_wait");
}

#[test]
fn node_op_id_resume_is_stable() {
    assert_eq!(node_op_id(&Node::resume("tag")), "vyre.node.resume");
}

#[test]
fn node_op_id_indirect_dispatch_is_stable() {
    assert_eq!(
        node_op_id(&Node::indirect_dispatch("buf", 0)),
        "vyre.node.indirect_dispatch"
    );
}

#[test]
fn node_op_id_collectives_are_stable() {
    let nodes = collective_nodes();
    let expected = [
        "vyre.node.all_reduce",
        "vyre.node.all_gather",
        "vyre.node.reduce_scatter",
        "vyre.node.broadcast",
    ];
    for (node, expected) in nodes.iter().zip(expected) {
        assert_eq!(
            node_op_id(node),
            expected,
            "distributed collective node IDs are frozen wire/dispatch contracts"
        );
    }
}

#[test]
fn node_op_id_is_deterministic_for_same_node() {
    let node = Node::barrier();
    assert_eq!(node_op_id(&node), node_op_id(&node));
}

#[test]
fn default_supported_ops_is_non_empty() {
    let ops = default_supported_ops();
    assert!(
        ops.contains(node_op_id(&Node::Return)),
        "default supported ops must include Return"
    );
}

#[test]
fn default_supported_ops_with_trap_is_superset() {
    let base = default_supported_ops();
    let with_trap = default_supported_ops_with_trap();
    for op in base.iter() {
        assert!(
            with_trap.contains(op),
            "with_trap must contain all base ops: {op}"
        );
    }
}

#[test]
fn default_supported_ops_excludes_distributed_collectives() {
    let ops = default_supported_ops();
    for node in collective_nodes() {
        let op = node_op_id(&node);
        assert!(
            !ops.contains(op),
            "distributed collective op {op} must require an explicit backend opt-in"
        );
    }
}

#[test]
fn default_supported_ops_with_trap_excludes_distributed_collectives() {
    let ops = default_supported_ops_with_trap();
    for node in collective_nodes() {
        let op = node_op_id(&node);
        assert!(
            !ops.contains(op),
            "trap support must not accidentally imply distributed collective support for {op}"
        );
    }
}

#[test]
fn default_backend_validation_rejects_distributed_collectives() {
    struct DefaultOpsBackend;

    impl vyre::backend::private::Sealed for DefaultOpsBackend {}

    impl VyreBackend for DefaultOpsBackend {
        fn id(&self) -> &'static str {
            "default-ops-contract"
        }

        fn supported_ops(&self) -> &HashSet<OpId> {
            default_supported_ops()
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(Vec::new())
        }
    }

    let program = Program::wrapped(vec![], [1, 1, 1], collective_nodes().into());
    let err = validate_program(&program, &DefaultOpsBackend).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("vyre.node.all_reduce"),
        "driver validation must reject the first unsupported collective explicitly; got: {msg}"
    );
}
