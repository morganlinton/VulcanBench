//! Substrate-neutral collective rewrites.

use std::fmt;
use std::sync::Arc;

use crate::ir::{CommGroup, Expr, Node, Program};

/// Error returned when a collective cannot be lowered without real transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleRankCollectiveError {
    fix: String,
}

impl SingleRankCollectiveError {
    fn new(fix: String) -> Self {
        Self { fix }
    }
}

impl fmt::Display for SingleRankCollectiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.fix)
    }
}

impl std::error::Error for SingleRankCollectiveError {}

/// How a collective node participates in transport planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectiveTransportKind {
    /// The collective has single-rank identity/copy semantics and can lower locally.
    LocalSingleRank,
    /// The collective needs a real transport such as NCCL, MPI, or UCX.
    MultiRankTransport,
}

/// Per-operation collective counts for backend transport planning.
///
/// NCCL/MPI/UCX lowering needs more than "some transport is required": launch
/// setup and communicator strategy differ between all-reduce, all-gather,
/// reduce-scatter, and broadcast. This histogram keeps that information in the
/// substrate-neutral plan without committing foundation to a transport runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectiveOpCounts {
    all_reduce: usize,
    all_gather: usize,
    reduce_scatter: usize,
    broadcast: usize,
}

impl CollectiveOpCounts {
    /// Number of all-reduce nodes.
    #[must_use]
    pub fn all_reduce(&self) -> usize {
        self.all_reduce
    }

    /// Number of all-gather nodes.
    #[must_use]
    pub fn all_gather(&self) -> usize {
        self.all_gather
    }

    /// Number of reduce-scatter nodes.
    #[must_use]
    pub fn reduce_scatter(&self) -> usize {
        self.reduce_scatter
    }

    /// Number of broadcast nodes.
    #[must_use]
    pub fn broadcast(&self) -> usize {
        self.broadcast
    }

    /// Total number of collective nodes represented by this histogram.
    #[must_use]
    pub fn total(&self) -> usize {
        self.all_reduce
            .saturating_add(self.all_gather)
            .saturating_add(self.reduce_scatter)
            .saturating_add(self.broadcast)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectiveNodeKind {
    AllReduce,
    AllGather,
    ReduceScatter,
    Broadcast,
}

/// Substrate-neutral execution plan for collectives in a program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollectiveTransportPlan {
    local_single_rank: usize,
    multi_rank_transport: usize,
    local_ops: CollectiveOpCounts,
    transport_ops: CollectiveOpCounts,
}

impl CollectiveTransportPlan {
    /// Return the number of collectives that can lower to local identity/copy IR.
    #[must_use]
    pub fn local_single_rank_collectives(&self) -> usize {
        self.local_single_rank
    }

    /// Return the number of collectives that require real multi-rank transport.
    #[must_use]
    pub fn transport_collectives(&self) -> usize {
        self.multi_rank_transport
    }

    /// Per-operation histogram for local single-rank collectives.
    #[must_use]
    pub fn local_ops(&self) -> CollectiveOpCounts {
        self.local_ops
    }

    /// Per-operation histogram for collectives requiring transport.
    #[must_use]
    pub fn transport_ops(&self) -> CollectiveOpCounts {
        self.transport_ops
    }

    /// Return true when at least one collective requires real transport.
    #[must_use]
    pub fn requires_transport(&self) -> bool {
        self.multi_rank_transport != 0
    }

    /// Return true when the program has no collective nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.local_single_rank == 0 && self.multi_rank_transport == 0
    }

    fn record(&mut self, transport: CollectiveTransportKind, node: CollectiveNodeKind) {
        match transport {
            CollectiveTransportKind::LocalSingleRank => {
                self.local_single_rank += 1;
                self.local_ops.record(node);
            }
            CollectiveTransportKind::MultiRankTransport => {
                self.multi_rank_transport += 1;
                self.transport_ops.record(node);
            }
        }
    }
}

impl CollectiveOpCounts {
    fn record(&mut self, kind: CollectiveNodeKind) {
        match kind {
            CollectiveNodeKind::AllReduce => self.all_reduce += 1,
            CollectiveNodeKind::AllGather => self.all_gather += 1,
            CollectiveNodeKind::ReduceScatter => self.reduce_scatter += 1,
            CollectiveNodeKind::Broadcast => self.broadcast += 1,
        }
    }
}

#[derive(Default)]
struct LoweringState {
    lowered: usize,
    next_copy_id: usize,
}

/// Lower collectives that are semantically single-rank.
///
/// `CommGroup::WORLD` with one participating rank has identity semantics for
/// `AllReduce` and root-0 `Broadcast`; `AllGather` and `ReduceScatter` become
/// bounded device copies. Any non-world group, or a nonzero single-rank
/// broadcast root, fails closed so transport-backed multi-rank collectives are
/// never silently emulated.
///
/// # Errors
///
/// Returns [`SingleRankCollectiveError`] when the program contains a collective
/// that needs real multi-rank transport.
pub fn lower_single_rank_collectives(
    program: &Program,
) -> Result<Option<Program>, SingleRankCollectiveError> {
    if !program.stats().distributed_collectives() {
        return Ok(None);
    }

    let mut state = LoweringState::default();
    let entry = rewrite_nodes(program.entry(), &mut state)?;
    if state.lowered == 0 {
        return Ok(None);
    }
    Ok(Some(program.with_rewritten_entry(entry)))
}

/// Return true when a program's collectives require real multi-rank transport.
///
/// This is intentionally weaker than "contains collective nodes": explicit
/// `CommGroup::WORLD` single-rank collectives can be lowered to local identity
/// or copy IR before backend emission. Non-world groups and nonzero broadcast
/// roots require a transport such as NCCL/MPI/UCX.
#[must_use]
pub fn requires_collective_transport(program: &Program) -> bool {
    collective_transport_plan(program).requires_transport()
}

/// Build the collective transport plan for a program.
///
/// This exposes backend-independent collective policy as data: WORLD
/// single-rank collectives can be emitted as local IR, while non-WORLD groups
/// and nonzero broadcast roots need a transport implementation.
#[must_use]
pub fn collective_transport_plan(program: &Program) -> CollectiveTransportPlan {
    let mut plan = CollectiveTransportPlan::default();
    record_nodes_transport_plan(program.entry(), &mut plan);
    plan
}

fn record_nodes_transport_plan(nodes: &[Node], plan: &mut CollectiveTransportPlan) {
    let mut stack = Vec::new();
    stack.push(nodes);
    while let Some(nodes) = stack.pop() {
        for node in nodes {
            match node {
                Node::AllReduce { group, .. } => {
                    plan.record(
                        transport_kind_for_group(*group),
                        CollectiveNodeKind::AllReduce,
                    );
                }
                Node::AllGather { group, .. } => {
                    plan.record(
                        transport_kind_for_group(*group),
                        CollectiveNodeKind::AllGather,
                    );
                }
                Node::ReduceScatter { group, .. } => {
                    plan.record(
                        transport_kind_for_group(*group),
                        CollectiveNodeKind::ReduceScatter,
                    );
                }
                Node::Broadcast { root, group, .. } => {
                    let transport = if *group == CommGroup::WORLD && *root == 0 {
                        CollectiveTransportKind::LocalSingleRank
                    } else {
                        CollectiveTransportKind::MultiRankTransport
                    };
                    plan.record(transport, CollectiveNodeKind::Broadcast);
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    stack.push(otherwise);
                    stack.push(then);
                }
                Node::Loop { body, .. } | Node::Block(body) => stack.push(body),
                Node::Region { body, .. } => stack.push(body.as_ref()),
                _ => {}
            }
        }
    }
}

fn transport_kind_for_group(group: CommGroup) -> CollectiveTransportKind {
    if group == CommGroup::WORLD {
        CollectiveTransportKind::LocalSingleRank
    } else {
        CollectiveTransportKind::MultiRankTransport
    }
}

fn rewrite_nodes(
    nodes: &[Node],
    state: &mut LoweringState,
) -> Result<Vec<Node>, SingleRankCollectiveError> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        out.extend(rewrite_node(node, state)?);
    }
    Ok(out)
}

fn rewrite_node(
    node: &Node,
    state: &mut LoweringState,
) -> Result<Vec<Node>, SingleRankCollectiveError> {
    match node {
        Node::AllReduce { group, .. } => {
            require_world_group(*group, "AllReduce")?;
            state.lowered += 1;
            Ok(Vec::new())
        }
        Node::Broadcast { root, group, .. } => {
            require_world_group(*group, "Broadcast")?;
            if *root != 0 {
                return Err(SingleRankCollectiveError::new(format!(
                    "Fix: single-rank Broadcast can only use root 0, got root {root}."
                )));
            }
            state.lowered += 1;
            Ok(Vec::new())
        }
        Node::AllGather {
            input,
            output,
            group,
        } => {
            require_world_group(*group, "AllGather")?;
            state.lowered += 1;
            Ok(single_rank_copy(input.as_str(), output.as_str(), state))
        }
        Node::ReduceScatter {
            input,
            output,
            group,
            ..
        } => {
            require_world_group(*group, "ReduceScatter")?;
            state.lowered += 1;
            Ok(single_rank_copy(input.as_str(), output.as_str(), state))
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => Ok(vec![Node::if_then_else(
            cond.clone(),
            rewrite_nodes(then, state)?,
            rewrite_nodes(otherwise, state)?,
        )]),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Ok(vec![Node::loop_for(
            var.shared_text(),
            from.clone(),
            to.clone(),
            rewrite_nodes(body, state)?,
        )]),
        Node::Block(children) => Ok(vec![Node::Block(rewrite_nodes(children, state)?)]),
        Node::Region {
            generator,
            source_region,
            body,
        } => Ok(vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rewrite_nodes(body.as_ref(), state)?),
        }]),
        other => Ok(vec![other.clone()]),
    }
}

fn require_world_group(group: CommGroup, op: &str) -> Result<(), SingleRankCollectiveError> {
    if group == CommGroup::WORLD {
        return Ok(());
    }
    Err(SingleRankCollectiveError::new(format!(
        "Fix: single-rank collective lowering only accepts CommGroup::WORLD for {op}, got group {}. Multi-rank collective transport must use a backend transport path instead of silent local emulation.",
        group.as_u32()
    )))
}

fn single_rank_copy(input: &str, output: &str, state: &mut LoweringState) -> Vec<Node> {
    let copy_id = state.next_copy_id;
    state.next_copy_id += 1;
    let idx = format!("__vyre_single_rank_collective_idx_{copy_id}");
    vec![
        Node::let_bind(idx.clone(), Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var(idx.clone()), Expr::buf_len(output.to_string())),
            vec![Node::if_then(
                Expr::lt(Expr::var(idx.clone()), Expr::buf_len(input.to_string())),
                vec![Node::store(
                    output.to_string(),
                    Expr::var(idx.clone()),
                    Expr::load(input.to_string(), Expr::var(idx)),
                )],
            )],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::ir::{BufferDecl, CollectiveOp, DataType};
    use crate::validate::validate;

    fn program_with(node: Node, count: u32) -> Program {
        Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(count),
                BufferDecl::output("out", 1, DataType::U32).with_count(count),
            ],
            [64, 1, 1],
            vec![node],
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        #[test]
        fn world_copy_collectives_lower_to_validation_safe_ir(count in 1u32..4096, reduce in any::<bool>()) {
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
            let program = program_with(node, count);
            let lowered = lower_single_rank_collectives(&program)
                .expect("Fix: WORLD single-rank collectives must lower")
                .expect("Fix: copy collective must produce replacement IR");

            prop_assert!(!lowered.stats().distributed_collectives());
            prop_assert!(validate(&lowered).is_empty());
            prop_assert!(!requires_collective_transport(&program));
            let plan = collective_transport_plan(&program);
            prop_assert_eq!(plan.local_single_rank_collectives(), 1);
            prop_assert_eq!(plan.transport_collectives(), 0);
            prop_assert!(!plan.requires_transport());
            prop_assert!(!plan.is_empty());
        }

        #[test]
        fn non_world_collectives_fail_closed(group in 1u32..4096) {
            let program = program_with(
                Node::AllGather {
                    input: "input".into(),
                    output: "out".into(),
                    group: CommGroup(group),
                },
                8,

            );

            let error = lower_single_rank_collectives(&program)
                .expect_err("non-WORLD group must not be silently emulated");
            prop_assert!(error.to_string().contains("Multi-rank collective transport"));
            prop_assert!(requires_collective_transport(&program));
            let plan = collective_transport_plan(&program);
            prop_assert_eq!(plan.local_single_rank_collectives(), 0);
            prop_assert_eq!(plan.transport_collectives(), 1);
            prop_assert!(plan.requires_transport());
        }

        #[test]
        fn nonzero_broadcast_root_fails_closed(root in 1u32..4096) {
            let program = program_with(
                Node::Broadcast {
                    buffer: "out".into(),
                    root,
                    group: CommGroup::WORLD,
                },
                8,
            );

            let error = lower_single_rank_collectives(&program)
                .expect_err("nonzero single-rank root must fail");
            prop_assert!(error.to_string().contains("root 0"));
            prop_assert!(requires_collective_transport(&program));
            let plan = collective_transport_plan(&program);
            prop_assert_eq!(plan.local_single_rank_collectives(), 0);
            prop_assert_eq!(plan.transport_collectives(), 1);
            prop_assert!(plan.requires_transport());
        }
    }

    #[test]
    fn transport_plan_counts_nested_local_and_transport_collectives() {
        let program = program_with(
            Node::Block(vec![
                Node::AllGather {
                    input: "input".into(),
                    output: "out".into(),
                    group: CommGroup::WORLD,
                },
                Node::Broadcast {
                    buffer: "out".into(),
                    root: 7,
                    group: CommGroup::WORLD,
                },
                Node::ReduceScatter {
                    input: "input".into(),
                    output: "out".into(),
                    op: CollectiveOp::Sum,
                    group: CommGroup(2),
                },
            ]),
            8,
        );

        let plan = collective_transport_plan(&program);

        assert_eq!(plan.local_single_rank_collectives(), 1);
        assert_eq!(plan.transport_collectives(), 2);
        assert_eq!(plan.local_ops().all_gather(), 1);
        assert_eq!(plan.local_ops().total(), 1);
        assert_eq!(plan.transport_ops().broadcast(), 1);
        assert_eq!(plan.transport_ops().reduce_scatter(), 1);
        assert_eq!(plan.transport_ops().total(), 2);
        assert!(plan.requires_transport());
        assert!(!plan.is_empty());
    }

    #[test]
    fn transport_plan_handles_deeply_nested_collectives_without_recursive_walk() {
        let mut node = Node::AllGather {
            input: "input".into(),
            output: "out".into(),
            group: CommGroup::WORLD,
        };
        for _ in 0..8192 {
            node = Node::Block(vec![node]);
        }
        let program = program_with(node, 8);

        let plan = collective_transport_plan(&program);

        assert_eq!(plan.local_single_rank_collectives(), 1);
        assert_eq!(plan.transport_collectives(), 0);
        assert_eq!(plan.local_ops().all_gather(), 1);
        assert!(!plan.requires_transport());
    }

    #[test]
    fn local_single_rank_lowering_covers_all_collective_node_kinds() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage(
                    "input",
                    0,
                    crate::ir::BufferAccess::ReadWrite,
                    DataType::U32,
                )
                .with_count(16),
                BufferDecl::storage("out", 1, crate::ir::BufferAccess::ReadWrite, DataType::U32)
                    .with_count(16),
            ],
            [64, 1, 1],
            vec![Node::Block(vec![
                Node::AllReduce {
                    buffer: "input".into(),
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
                    group: CommGroup::WORLD,
                },
                Node::Broadcast {
                    buffer: "out".into(),
                    root: 0,
                    group: CommGroup::WORLD,
                },
            ])],
        );
        let plan = collective_transport_plan(&program);
        assert_eq!(plan.local_single_rank_collectives(), 4);
        assert_eq!(plan.transport_collectives(), 0);
        assert_eq!(plan.local_ops().all_reduce(), 1);
        assert_eq!(plan.local_ops().all_gather(), 1);
        assert_eq!(plan.local_ops().reduce_scatter(), 1);
        assert_eq!(plan.local_ops().broadcast(), 1);

        let lowered = lower_single_rank_collectives(&program)
            .expect("Fix: all WORLD single-rank collective kinds must lower locally")
            .expect("Fix: local collective lowering must rewrite the program");

        assert!(!lowered.stats().distributed_collectives());
        assert!(validate(&lowered).is_empty());
    }

    #[test]
    fn generated_collective_transport_plan_histograms_classify_all_kinds() {
        for seed in 0..4096u32 {
            let mut expected_local = CollectiveOpCounts::default();
            let mut expected_transport = CollectiveOpCounts::default();
            let mut nodes = Vec::with_capacity(16);

            for offset in 0..16u32 {
                let selector = seed.wrapping_mul(31).wrapping_add(offset * 17);
                let force_transport = selector.rotate_left(offset % 13) & 0x4 != 0;
                let (node, kind, is_local) = generated_collective_node(selector, force_transport);
                if is_local {
                    expected_local.record(kind);
                } else {
                    expected_transport.record(kind);
                }
                if offset % 5 == 0 {
                    nodes.push(Node::Block(vec![node]));
                } else {
                    nodes.push(node);
                }
            }

            let program = Program::wrapped(
                vec![
                    BufferDecl::storage(
                        "input",
                        0,
                        crate::ir::BufferAccess::ReadWrite,
                        DataType::U32,
                    )
                    .with_count(16),
                    BufferDecl::storage(
                        "out",
                        1,
                        crate::ir::BufferAccess::ReadWrite,
                        DataType::U32,
                    )
                    .with_count(16),
                ],
                [64, 1, 1],
                vec![Node::Block(nodes)],
            );
            let plan = collective_transport_plan(&program);

            assert_eq!(plan.local_ops(), expected_local, "seed={seed}");
            assert_eq!(plan.transport_ops(), expected_transport, "seed={seed}");
            assert_eq!(
                plan.local_single_rank_collectives(),
                expected_local.total(),
                "seed={seed}"
            );
            assert_eq!(
                plan.transport_collectives(),
                expected_transport.total(),
                "seed={seed}"
            );
            assert_eq!(plan.requires_transport(), expected_transport.total() != 0);
        }
    }

    fn generated_collective_node(
        selector: u32,
        force_transport: bool,
    ) -> (Node, CollectiveNodeKind, bool) {
        let group = if force_transport {
            CommGroup(selector | 1)
        } else {
            CommGroup::WORLD
        };
        match selector % 4 {
            0 => (
                Node::AllReduce {
                    buffer: "input".into(),
                    op: CollectiveOp::Sum,
                    group,
                },
                CollectiveNodeKind::AllReduce,
                group == CommGroup::WORLD,
            ),
            1 => (
                Node::AllGather {
                    input: "input".into(),
                    output: "out".into(),
                    group,
                },
                CollectiveNodeKind::AllGather,
                group == CommGroup::WORLD,
            ),
            2 => (
                Node::ReduceScatter {
                    input: "input".into(),
                    output: "out".into(),
                    op: CollectiveOp::Max,
                    group,
                },
                CollectiveNodeKind::ReduceScatter,
                group == CommGroup::WORLD,
            ),
            _ => {
                let root = if force_transport { selector % 7 + 1 } else { 0 };
                (
                    Node::Broadcast {
                        buffer: "out".into(),
                        root,
                        group,
                    },
                    CollectiveNodeKind::Broadcast,
                    group == CommGroup::WORLD && root == 0,
                )
            }
        }
    }
}
