//! Reference-oracle coverage for substrate-neutral single-rank collectives.

mod common;
use common::{bytes_to_u32, u32_bytes};
use proptest::prelude::*;
use vyre_foundation::ir::{BufferDecl, CollectiveOp, CommGroup, DataType, Expr, Node, Program};
use vyre_reference::{reference_eval, value::Value};

fn copy_program(node: Node, count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(count),
            BufferDecl::output("out", 1, DataType::U32).with_count(count),
        ],
        [64, 1, 1],
        vec![node],
    )
}

fn identity_program(node: Node, count: u32) -> Program {
    let idx = Expr::gid_x();
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(count),
            BufferDecl::output("out", 1, DataType::U32).with_count(count),
        ],
        [count, 1, 1],
        vec![
            Node::if_then(
                Expr::lt(idx.clone(), Expr::u32(count)),
                vec![Node::store(
                    "out",
                    idx.clone(),
                    Expr::load("input", idx.clone()),
                )],
            ),
            node,
        ],
    )
}

fn collective_shape(kind: u32, group: CommGroup, root: u32) -> Node {
    match kind % 4 {
        0 => Node::AllReduce {
            buffer: "out".into(),
            op: CollectiveOp::Sum,
            group,
        },
        1 => Node::AllGather {
            input: "input".into(),
            output: "out".into(),
            group,
        },
        2 => Node::ReduceScatter {
            input: "input".into(),
            output: "out".into(),
            op: CollectiveOp::Max,
            group,
        },
        _ => Node::Broadcast {
            buffer: "out".into(),
            root,
            group,
        },
    }
}

#[test]
fn subgroup_shuffle_observes_branch_assigned_source_lane_after_empty_peer_branch() {
    let lane = Expr::LocalId { axis: 0 };
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(32),
        ],
        [32, 1, 1],
        vec![
            Node::let_bind("leader_word", Expr::u32(0)),
            Node::if_then(
                Expr::eq(lane.clone(), Expr::u32(0)),
                vec![Node::assign(
                    "leader_word",
                    Expr::load("input", Expr::u32(0)),
                )],
            ),
            Node::store(
                "out",
                lane,
                Expr::subgroup_shuffle(Expr::var("leader_word"), Expr::u32(0)),
            ),
        ],
    );

    let outputs = reference_eval(&program, &[Value::from(u32_bytes(&[0xfeed_cafe]))])
        .expect("Fix: reference oracle must execute branch-fed subgroup shuffle.");

    assert_eq!(bytes_to_u32(&outputs[0]), vec![0xfeed_cafe; 32]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn reference_executes_world_copy_collectives(values in proptest::collection::vec(any::<u32>(), 1..256), reduce in any::<bool>()) {
        let count = values.len() as u32;
        let node = if reduce {
            Node::ReduceScatter {
                input: "input".into(),
                output: "out".into(),
                op: CollectiveOp::Sum,
                group: CommGroup::WORLD,
            }
        } else {
            Node::AllGather {
                input: "input".into(),
                output: "out".into(),
                group: CommGroup::WORLD,
            }
        };
        let program = copy_program(node, count);
        let outputs = reference_eval(&program, &[Value::from(u32_bytes(&values))])
            .expect("Fix: reference oracle must execute substrate-neutral single-rank collectives.");

        prop_assert_eq!(outputs.len(), 1);
        prop_assert_eq!(bytes_to_u32(&outputs[0]), values);
    }

    #[test]
    fn reference_executes_world_identity_collectives(values in proptest::collection::vec(any::<u32>(), 1..256), all_reduce in any::<bool>()) {
        let count = values.len() as u32;
        let node = if all_reduce {
            Node::AllReduce {
                buffer: "out".into(),
                op: CollectiveOp::Sum,
                group: CommGroup::WORLD,
            }
        } else {
            Node::Broadcast {
                buffer: "out".into(),
                root: 0,
                group: CommGroup::WORLD,
            }
        };
        let program = identity_program(node, count);
        let outputs = reference_eval(&program, &[Value::from(u32_bytes(&values))])
            .expect("Fix: reference oracle must execute WORLD identity collectives by lowering them locally.");

        prop_assert_eq!(outputs.len(), 1);
        prop_assert_eq!(bytes_to_u32(&outputs[0]), values);
    }

    #[test]
    fn reference_rejects_non_world_collectives(group in 1u32..4096, kind in 0u32..4) {
        let program = copy_program(collective_shape(kind, CommGroup(group), 0), 4);

        let error = reference_eval(&program, &[Value::from(u32_bytes(&[1, 2, 3, 4]))])
            .expect_err("Fix: reference oracle must not silently emulate multi-rank collectives.");
        prop_assert!(error.to_string().contains("Multi-rank collective transport"));
    }

    #[test]
    fn reference_rejects_nonzero_world_broadcast_root(root in 1u32..4096) {
        let program = identity_program(
            Node::Broadcast {
                buffer: "out".into(),
                root,
                group: CommGroup::WORLD,
            },
            4,
        );

        let error = reference_eval(&program, &[Value::from(u32_bytes(&[1, 2, 3, 4]))])
            .expect_err("Fix: reference oracle must not silently emulate single-rank broadcast from a nonzero root.");
        prop_assert!(error.to_string().contains("Broadcast can only use root 0"));
    }
}
