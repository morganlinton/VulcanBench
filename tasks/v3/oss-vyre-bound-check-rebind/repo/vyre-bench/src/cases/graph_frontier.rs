use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

mod skewed_csr;

const NODE_COUNT: u32 = 1_048_576;
const EDGE_PERMUTATION_MULTIPLIER: u32 = 1_103_515_245;
const EDGE_PERMUTATION_INCREMENT: u32 = 12_345;
const EDGE_MASK: u32 = NODE_COUNT - 1;

struct GraphFrontierPrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    active_vertices: u64,
    resident: Option<ResidentInputSet>,
}

/// Million-node graph frontier expansion with exact CPU oracle.
pub struct GraphFrontierStep;

impl BenchCase for GraphFrontierStep {
    fn id(&self) -> BenchId {
        BenchId("primitives.graph.frontier_step.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Graph Frontier Step 1M".to_string(),
            description:
                "Single-hop graph frontier expansion over a million-node permutation graph using deterministic inverse-edge gather"
                    .to_string(),
            tags: vec![
                "graph".to_string(),
                "frontier".to_string(),
                "pointer-chasing".to_string(),
                "scatter".to_string(),
                "branching".to_string(),
            ],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(64 * 1024 * 1024),
            min_input_bytes: Some(NODE_COUNT as u64 * 12),
            feature_set: vec!["graph.frontier".to_string()],
        }
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        (NODE_COUNT as u64 * 8, NODE_COUNT as u64 * 4)
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_graph_frontier_case(Some(ctx))?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<GraphFrontierPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<GraphFrontierPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared graph frontier payload had the wrong type".to_string(),
                )
            })?;

        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let timed = dispatch.timed;
        let output_bytes_total = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(
            prepared.input_bytes_total,
            output_bytes_total,
            resident_used,
        );

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes_total),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: vec![
                    MetricPoint {
                        name: "graph_nodes".to_string(),
                        value: NODE_COUNT as u64,
                    },
                    MetricPoint {
                        name: "graph_edges".to_string(),
                        value: NODE_COUNT as u64,
                    },
                    MetricPoint {
                        name: "active_frontier_vertices".to_string(),
                        value: prepared.active_vertices,
                    },
                    MetricPoint {
                        name: "resident_buffers".to_string(),
                        value: u64::from(prepared.resident.is_some()),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(
                    prepared.inputs[0].len() as u64 + prepared.inputs[1].len() as u64,
                ),
                output_bytes: Some(NODE_COUNT as u64 * 4),
                custom: vec![MetricPoint {
                    name: "active_frontier_vertices".to_string(),
                    value: prepared.active_vertices,
                }],
                ..Default::default()
            }),
            outputs: timed.outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn prepare_graph_frontier_case(
    ctx: Option<&BenchContext>,
) -> Result<GraphFrontierPrepared, BenchError> {
    let program = graph_frontier_program();
    let inputs = graph_frontier_inputs();
    let input_bytes_total = input_bytes_total(&inputs);
    let active_vertices = active_frontier_count(&inputs[0]);
    let start_ref = std::time::Instant::now();
    let baseline_output = graph_frontier_cpu_oracle(&inputs[0], &inputs[1]);
    let baseline_wall_ns = start_ref.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    let resident = ctx
        .map(|ctx| {
            ResidentInputSet::upload_with_zeroed_outputs_optional(
                ctx,
                &inputs,
                &[NODE_COUNT as usize * 4],
                "graph frontier",
            )
        })
        .transpose()?
        .flatten();

    Ok(GraphFrontierPrepared {
        program,
        inputs,
        input_bytes_total,
        baseline_output,
        baseline_wall_ns,
        active_vertices,
        resident,
    })
}

fn graph_frontier_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("frontier", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(NODE_COUNT),
            BufferDecl::storage("predecessors", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(NODE_COUNT),
            BufferDecl::output("next", 2, DataType::U32).with_count(NODE_COUNT),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("dst", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("dst"), Expr::u32(NODE_COUNT)),
                vec![
                    Node::let_bind("src", Expr::load("predecessors", Expr::var("dst"))),
                    Node::store(
                        "next",
                        Expr::var("dst"),
                        Expr::select(
                            Expr::lt(Expr::u32(0), Expr::load("frontier", Expr::var("src"))),
                            Expr::u32(1),
                            Expr::u32(0),
                        ),
                    ),
                ],
            ),
        ],
    )
}

fn graph_frontier_inputs() -> Vec<Vec<u8>> {
    let mut frontier = vec![0_u8; NODE_COUNT as usize * 4];
    let mut predecessors = vec![0_u8; NODE_COUNT as usize * 4];
    for index in 0..NODE_COUNT {
        let active = u32::from(index % 17 == 0 || index == NODE_COUNT / 2);
        write_u32(&mut frontier, index, active);
        write_u32(&mut predecessors, permutation_edge(index), index);
    }
    vec![frontier, predecessors]
}

fn graph_frontier_cpu_oracle(frontier: &[u8], predecessors: &[u8]) -> Vec<u8> {
    let mut next = vec![0_u8; NODE_COUNT as usize * 4];
    for dst in 0..NODE_COUNT {
        let src = read_u32(predecessors, dst);
        write_u32(&mut next, dst, u32::from(read_u32(frontier, src) != 0));
    }
    next
}

fn active_frontier_count(frontier: &[u8]) -> u64 {
    let mut active = 0_u64;
    for index in 0..NODE_COUNT {
        active += u64::from(read_u32(frontier, index) != 0);
    }
    active
}

fn permutation_edge(index: u32) -> u32 {
    index
        .wrapping_mul(EDGE_PERMUTATION_MULTIPLIER)
        .wrapping_add(EDGE_PERMUTATION_INCREMENT)
        & EDGE_MASK
}

fn read_u32(bytes: &[u8], index: u32) -> u32 {
    let start = index as usize * 4;
    u32::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
    ])
}

fn write_u32(bytes: &mut [u8], index: u32, value: u32) {
    let start = index as usize * 4;
    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
}

inventory::submit! {
    &GraphFrontierStep as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_frontier_permutation_has_no_duplicate_destinations() {
        let mut seen = vec![false; NODE_COUNT as usize];
        for index in 0..NODE_COUNT {
            let dst = permutation_edge(index);
            assert!(
                !seen[dst as usize],
                "permutation edge collision at source {index} destination {dst}"
            );
            seen[dst as usize] = true;
        }
    }

    #[test]
    fn graph_frontier_cpu_oracle_sets_one_bit_per_active_vertex() {
        let inputs = graph_frontier_inputs();
        let active = active_frontier_count(&inputs[0]);
        let oracle = graph_frontier_cpu_oracle(&inputs[0], &inputs[1]);
        let mut set_bits = 0_u64;
        for index in 0..NODE_COUNT {
            set_bits += u64::from(read_u32(&oracle, index) != 0);
        }

        assert_eq!(set_bits, active);
    }

    #[test]
    fn graph_frontier_inputs_encode_inverse_permutation_edges() {
        let inputs = graph_frontier_inputs();
        for source in 0..NODE_COUNT {
            let dst = permutation_edge(source);
            assert_eq!(
                read_u32(&inputs[1], dst),
                source,
                "predecessor table must let each destination lane write a deterministic output slot"
            );
        }
    }

    #[test]
    fn graph_frontier_prepare_caches_oracle_and_program() {
        let prepared = prepare_graph_frontier_case(None).unwrap();
        assert_eq!(prepared.program.buffers()[0].name(), "frontier");
        assert_eq!(prepared.program.buffers()[1].name(), "predecessors");
        assert_eq!(prepared.program.buffers()[2].name(), "next");
        assert_eq!(
            prepared.inputs.iter().map(Vec::len).sum::<usize>(),
            NODE_COUNT as usize * 8
        );
        assert_eq!(prepared.baseline_output.len(), NODE_COUNT as usize * 4);
        assert_eq!(
            prepared.active_vertices,
            active_frontier_count(&prepared.inputs[0])
        );
        assert_eq!(
            prepared.baseline_output,
            graph_frontier_cpu_oracle(&prepared.inputs[0], &prepared.inputs[1])
        );

        let boxed: PreparedCase = Box::new(prepared);
        assert!(GraphFrontierStep.program(&boxed).is_some());
    }
}
