//! Generated adversarial coverage for the public storage-graph reference oracle.

use std::collections::BTreeMap;

use vyre_foundation::ir::{BinOp, NodeId, NodeStorage, Value as IrValue};
use vyre_reference::run_storage_graph;

#[test]
fn generated_storage_graph_oracle_matches_recursive_shadow_for_32768_cases() {
    let mut rng = 0x6a09_e667_f3bc_c908_u64;
    for case in 0..32_768u32 {
        let graph = generated_acyclic_graph(&mut rng, case);
        let output = graph
            .last()
            .expect("Fix: generated storage graph must be non-empty.")
            .0;
        let expected = recursive_value(output, &graph)
            .expect("Fix: generated acyclic storage graph must evaluate recursively.");
        let actual = run_storage_graph(&graph, &[output])
            .expect("Fix: public storage graph oracle must evaluate generated acyclic graph.");
        assert_eq!(actual, vec![IrValue::U32(expected)], "case {case}");
    }
}

#[test]
fn generated_storage_graph_oracle_handles_unordered_multi_output_dags_for_16384_cases() {
    let mut rng = 0xbb67_ae85_84ca_a73b_u64;
    for case in 0..16_384u32 {
        let graph = generated_acyclic_graph(&mut rng, case ^ 0xA5A5_5A5A);
        let output_ids = generated_outputs(&graph, &mut rng);
        let expected = output_ids
            .iter()
            .map(|id| {
                recursive_value(*id, &graph).map(IrValue::U32).expect(
                    "Fix: generated acyclic storage graph output must evaluate recursively.",
                )
            })
            .collect::<Vec<_>>();
        let mut unordered = graph.clone();
        shuffle_graph(&mut unordered, &mut rng);

        let actual = run_storage_graph(&unordered, &output_ids)
            .expect("Fix: public storage graph oracle must evaluate unordered generated DAGs.");

        assert_eq!(actual, expected, "case {case}");
    }
}

#[test]
fn storage_graph_public_oracle_reports_missing_dependencies_and_cycles() {
    let missing = vec![(
        NodeId(0),
        NodeStorage::BinOp {
            op: BinOp::Add,
            left: NodeId(1),
            right: NodeId(2),
        },
    )];
    let missing_error = run_storage_graph(&missing, &[NodeId(0)])
        .expect_err("Fix: storage graph with missing inputs must fail.");
    assert!(
        missing_error.to_string().contains("missing node"),
        "Fix: missing dependency errors must identify the graph-shape bug: {missing_error}"
    );

    let cycle = vec![
        (
            NodeId(0),
            NodeStorage::BinOp {
                op: BinOp::Add,
                left: NodeId(1),
                right: NodeId(1),
            },
        ),
        (
            NodeId(1),
            NodeStorage::BinOp {
                op: BinOp::BitXor,
                left: NodeId(0),
                right: NodeId(0),
            },
        ),
    ];
    let cycle_error =
        run_storage_graph(&cycle, &[NodeId(0)]).expect_err("Fix: cyclic storage graph must fail.");
    assert!(
        cycle_error.to_string().contains("cycle"),
        "Fix: cycle errors must identify the graph-shape bug: {cycle_error}"
    );
}

#[test]
fn storage_graph_public_oracle_rejects_duplicate_node_ids() {
    let graph = vec![
        (NodeId(0), NodeStorage::LitU32(1)),
        (NodeId(0), NodeStorage::LitU32(2)),
    ];
    let error = run_storage_graph(&graph, &[NodeId(0)])
        .expect_err("Fix: duplicate NodeId records must fail instead of silently overwriting.");

    assert!(
        error.to_string().contains("duplicate node"),
        "Fix: duplicate node errors must identify the graph-shape bug: {error}"
    );
}

fn generated_acyclic_graph(rng: &mut u64, case: u32) -> Vec<(NodeId, NodeStorage)> {
    let len = 2 + (next(rng) as usize % 63);
    let mut graph = Vec::with_capacity(len);
    graph.push((NodeId(0), NodeStorage::LitU32(case.rotate_left(case % 31))));
    graph.push((NodeId(1), NodeStorage::LitU32(next(rng))));
    for index in 2..len {
        let left = NodeId(next(rng) % index as u32);
        let right = NodeId(next(rng) % index as u32);
        graph.push((
            NodeId(index as u32),
            NodeStorage::BinOp {
                op: generated_op(next(rng)),
                left,
                right,
            },
        ));
    }
    graph
}

fn generated_op(seed: u32) -> BinOp {
    match seed % 5 {
        0 => BinOp::Add,
        1 => BinOp::Sub,
        2 => BinOp::Mul,
        3 => BinOp::BitXor,
        _ => BinOp::BitAnd,
    }
}

fn generated_outputs(graph: &[(NodeId, NodeStorage)], rng: &mut u64) -> Vec<NodeId> {
    let len = graph.len() as u32;
    let mut outputs = Vec::with_capacity(6);
    outputs.push(graph.last().expect("Fix: generated graph is non-empty.").0);
    outputs.push(NodeId(next(rng) % len));
    outputs.push(NodeId(next(rng) % len));
    outputs.push(NodeId(0));
    outputs.push(NodeId((len - 1) / 2));
    outputs.push(NodeId(len - 1));
    outputs
}

fn shuffle_graph(graph: &mut [(NodeId, NodeStorage)], rng: &mut u64) {
    for index in (1..graph.len()).rev() {
        let swap = next(rng) as usize % (index + 1);
        graph.swap(index, swap);
    }
}

fn recursive_value(id: NodeId, graph: &[(NodeId, NodeStorage)]) -> Result<u32, &'static str> {
    let by_id = graph
        .iter()
        .map(|(node_id, node)| (*node_id, node))
        .collect::<BTreeMap<_, _>>();
    recursive_value_inner(id, &by_id, &mut Vec::new())
}

fn recursive_value_inner(
    id: NodeId,
    graph: &BTreeMap<NodeId, &NodeStorage>,
    visiting: &mut Vec<NodeId>,
) -> Result<u32, &'static str> {
    if visiting.contains(&id) {
        return Err("cycle");
    }
    let node = graph.get(&id).ok_or("missing")?;
    match node {
        NodeStorage::LitU32(value) => Ok(*value),
        NodeStorage::BinOp { op, left, right } => {
            visiting.push(id);
            let left = recursive_value_inner(*left, graph, visiting)?;
            let right = recursive_value_inner(*right, graph, visiting)?;
            visiting.pop();
            match op {
                BinOp::Add => Ok(left.wrapping_add(right)),
                BinOp::Sub => Ok(left.wrapping_sub(right)),
                BinOp::Mul => Ok(left.wrapping_mul(right)),
                BinOp::BitXor => Ok(left ^ right),
                BinOp::BitAnd => Ok(left & right),
                _ => Err("unsupported"),
            }
        }
        _ => Err("unsupported"),
    }
}

fn next(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 32) as u32
}
