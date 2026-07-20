//! `level_wave_program`  -  GPU-resident depth-wave dispatcher for
//! bottom-up callee-before-caller computations.
//!
//! Semantically distinct from
//! [`fixpoint::persistent_fixpoint`](crate::fixpoint::persistent_fixpoint):
//! - **persistent_fixpoint**: re-run a transfer step until convergence.
//!   No depth ordering  -  every lane runs the step every iteration.
//! - **level_wave**: deterministic ordered traversal. Each lane runs
//!   the step only when `current_depth == depth`lane``. Used for
//!   bottom-up summary computations where children must complete
//!   before parents.
//!
//! ## LEGO discipline
//!
//! Composes:
//! - [`crate::graph::toposort::toposort()`]  -  CPU reference for the depth
//!   assignment (caller computes `depth[node]` from the topological
//!   ordering before invoking this primitive).
//! - `Node::Loop` (vyre-foundation IR primitive)  -  outer per-depth
//!   loop.
//! - `Node::Barrier { ordering: vyre_foundation::MemoryOrdering::SeqCst }`  -  synchronisation between depth waves.
//! - `Expr::eq` + `Node::if_then`  -  depth predicate per lane.
//!
//! No new sub-op invented. The caller composes its own per-lane work
//! body; this primitive provides the wave harness.
//!
//! ## Composition contract
//!
//! Caller supplies:
//!
//! - `depth_buf`: per-node depth bitset (u32 per lane). The lane
//!   reads its own depth and gates its work on equality with the
//!   current wave depth.
//! - `step_body`: caller-provided IR body that runs ONE node's work.
//!   Reads `current_depth` and `depth_buf`lane``; the
//!   level_wave_program guards the body in `if depth == current`
//!   already, so the body itself doesn't need to re-check.
//! - `max_depth`: maximum depth value in the topology.
//!
//! Caller receives a `Program` that runs every lane at every depth wave
//! from 0..max_depth. Single-workgroup waves use one compact loop.
//! Multi-workgroup waves expose top-level `GridSync` boundaries so
//! backends without native grid barriers can split the traversal into
//! launch-separated depth waves.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{
    BufferAccess, BufferDecl, DataType, Expr, MemoryOrdering, Node, Program,
};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::level_wave";
/// Workgroup shape for per-node depth-wave traversal.
pub const LEVEL_WAVE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid that covers every level-wave lane.
#[must_use]
pub const fn level_wave_dispatch_grid(lane_count: u32) -> [u32; 3] {
    let blocks = lane_count.div_ceil(LEVEL_WAVE_WORKGROUP_SIZE[0]);
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

fn depth_wave_body(
    step_body: Vec<Node>,
    depth_buf: &str,
    depth: Expr,
    lane_count: u32,
) -> Vec<Node> {
    let lane = Expr::InvocationId { axis: 0 };
    let depth_for_lane = Expr::load(depth_buf, lane.clone());
    vec![Node::if_then(
        Expr::and(
            Expr::lt(lane, Expr::u32(lane_count)),
            Expr::eq(depth_for_lane, depth),
        ),
        step_body,
    )]
}

/// Build a Program that runs `step_body` per lane in
/// depth-ordered waves.
///
/// Each lane reads `depth_buf[invocation_id]`. The kernel walks
/// `current_depth = 0..max_depth`. At each depth, every lane whose
/// depth equals `current_depth` executes `step_body`. A `Barrier` is
/// emitted between depths so the caller can rely on depth-N effects
/// being globally visible before depth-N+1 begins.
///
/// # Parameters
///
/// - `step_body`: caller's per-lane work body. Free to read/write
///   any buffer the caller declares; it does NOT need to re-check
///   the depth predicate (the wrapper does that).
/// - `depth_buf`: buffer-name holding per-lane depth (u32). Read-only.
/// - `max_depth`: number of waves to execute.
/// - `lane_count`: total number of lanes in the dispatch grid.
#[must_use]
pub fn level_wave_program(
    step_body: Vec<Node>,
    depth_buf: &str,
    max_depth: u32,
    lane_count: u32,
) -> Program {
    let body = if lane_count <= LEVEL_WAVE_WORKGROUP_SIZE[0] {
        vec![Node::loop_for(
            "__lw_depth__",
            Expr::u32(0),
            Expr::u32(max_depth),
            {
                let mut loop_body = depth_wave_body(
                    step_body.clone(),
                    depth_buf,
                    Expr::var("__lw_depth__"),
                    lane_count,
                );
                loop_body.push(Node::Barrier {
                    ordering: MemoryOrdering::SeqCst,
                });
                loop_body
            },
        )]
    } else {
        let mut waves = Vec::with_capacity(max_depth.saturating_mul(2) as usize);
        for depth in 0..max_depth {
            waves.extend(depth_wave_body(
                step_body.clone(),
                depth_buf,
                Expr::u32(depth),
                lane_count,
            ));
            if depth + 1 < max_depth {
                waves.push(Node::Barrier {
                    ordering: MemoryOrdering::GridSync,
                });
            }
        }
        waves
    };

    Program::wrapped(
        vec![
            BufferDecl::storage(depth_buf, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count),
        ],
        LEVEL_WAVE_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU oracle. Iterates depth waves on the host and calls
/// `step_for_lane(lane, depth)` exactly once per (lane, depth ==
/// depth_for_lane`lane`). Used by the conformance harness to verify
/// that the GPU kernel respects the depth ordering.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref<F>(depths: &[u32], max_depth: u32, mut step_for_lane: F)
where
    F: FnMut(u32, u32),
{
    for current_depth in 0..max_depth {
        for (lane_idx, lane_depth) in depths.iter().enumerate() {
            if *lane_depth == current_depth {
                step_for_lane(lane_idx as u32, current_depth);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_region_body(program: &Program) -> &[Node] {
        match &program.entry()[0] {
            Node::Region { body, .. } => body.as_slice(),
            other => panic!("expected wrapped level-wave region, got {other:?}"),
        }
    }

    fn contains_grid_sync(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            } => true,
            Node::Block(children) | Node::Loop { body: children, .. } => {
                contains_grid_sync(children)
            }
            Node::If {
                then, otherwise, ..
            } => contains_grid_sync(then) || contains_grid_sync(otherwise),
            Node::Region { body, .. } => contains_grid_sync(body),
            _ => false,
        })
    }

    fn contains_loop(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Loop { .. } => true,
            Node::Block(children) => contains_loop(children),
            Node::If {
                then, otherwise, ..
            } => contains_loop(then) || contains_loop(otherwise),
            Node::Region { body, .. } => contains_loop(body),
            _ => false,
        })
    }

    #[test]
    fn cpu_ref_visits_each_lane_at_its_depth() {
        let depths = vec![0u32, 1, 2, 1, 0];
        let mut visits: Vec<(u32, u32)> = Vec::new();
        cpu_ref(&depths, 3, |lane, depth| visits.push((lane, depth)));
        // Every lane visited exactly once, in depth order.
        assert_eq!(visits.len(), depths.len());
        for (idx, &(lane, depth)) in visits.iter().enumerate() {
            assert_eq!(depth, depths[lane as usize]);
            // Visits are sorted by depth (waves).
            if idx > 0 {
                assert!(depth >= visits[idx - 1].1);
            }
        }
    }

    #[test]
    fn dispatch_grid_packs_lane_count_into_workgroups() {
        assert_eq!(level_wave_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(level_wave_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(level_wave_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(level_wave_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(level_wave_dispatch_grid(1029), [5, 1, 1]);
    }

    #[test]
    fn program_shape_matches_contract() {
        let step = vec![Node::store("out", Expr::u32(0), Expr::u32(1))];
        let program = level_wave_program(step, "depths", 8, 64);
        assert_eq!(program.workgroup_size(), LEVEL_WAVE_WORKGROUP_SIZE);
        assert!(
            program.buffers.iter().any(|b| b.name() == "depths"),
            "depth buffer must be declared"
        );
        assert!(!contains_grid_sync(entry_region_body(&program)));
    }

    #[test]
    fn multi_block_program_uses_top_level_grid_sync_waves() {
        let step = vec![Node::store(
            "out",
            Expr::InvocationId { axis: 0 },
            Expr::u32(1),
        )];
        let program = level_wave_program(step, "depths", 4, LEVEL_WAVE_WORKGROUP_SIZE[0] + 1);
        let body = entry_region_body(&program);
        assert!(contains_grid_sync(body));
        assert!(
            !contains_loop(body),
            "multi-block level-wave must expose GridSync at split-visible depth-wave boundaries"
        );
        assert_eq!(
            body.iter()
                .filter(|node| matches!(
                    node,
                    Node::Barrier {
                        ordering: MemoryOrdering::GridSync,
                    }
                ))
                .count(),
            3
        );
    }
}
