//! Value-set interval boundary propagation as Vyre IR.
//!
//! The primitive computes conservative `[min, max]` interval merges over u32
//! pairs. Backend crates decide how to lower min/max; this module only owns the
//! substrate-neutral program shape.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for interval merge.
pub const OP_ID: &str = "vyre-primitives::math::interval_merge";

/// Build the per-lane interval merge body.
///
/// Buffers use pair layout: `mins_out[i] = min(mins_a[i], mins_b[i])` and
/// `maxs_out[i] = max(maxs_a[i], maxs_b[i])`.
#[must_use]
pub fn interval_merge_body(
    mins_a: &str,
    maxs_a: &str,
    mins_b: &str,
    maxs_b: &str,
    mins_out: &str,
    maxs_out: &str,
    lane_count: u32,
) -> Vec<Node> {
    let lane = Expr::gid_x();
    vec![Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(lane_count)),
        vec![
            Node::let_bind("interval_min_a", Expr::load(mins_a, lane.clone())),
            Node::let_bind("interval_max_a", Expr::load(maxs_a, lane.clone())),
            Node::let_bind("interval_min_b", Expr::load(mins_b, lane.clone())),
            Node::let_bind("interval_max_b", Expr::load(maxs_b, lane.clone())),
            Node::store(
                mins_out,
                lane.clone(),
                Expr::min(Expr::var("interval_min_a"), Expr::var("interval_min_b")),
            ),
            Node::store(
                maxs_out,
                lane,
                Expr::max(Expr::var("interval_max_a"), Expr::var("interval_max_b")),
            ),
        ],
    )]
}

/// Build an interval merge Program.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn interval_merge_program(
    mins_a: &str,
    maxs_a: &str,
    mins_b: &str,
    maxs_b: &str,
    mins_out: &str,
    maxs_out: &str,
    lane_count: u32,
) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(mins_a, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count.max(1)),
            BufferDecl::storage(maxs_a, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count.max(1)),
            BufferDecl::storage(mins_b, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count.max(1)),
            BufferDecl::storage(maxs_b, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count.max(1)),
            BufferDecl::storage(mins_out, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(lane_count.max(1)),
            BufferDecl::storage(maxs_out, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(lane_count.max(1)),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(interval_merge_body(
                mins_a, maxs_a, mins_b, maxs_b, mins_out, maxs_out, lane_count,
            )),
        }],
    )
}

/// CPU oracle for interval merge.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_interval_merge(
    mins_a: &[u32],
    maxs_a: &[u32],
    mins_b: &[u32],
    maxs_b: &[u32],
) -> (Vec<u32>, Vec<u32>) {
    let len = mins_a
        .len()
        .min(maxs_a.len())
        .min(mins_b.len())
        .min(maxs_b.len());
    let mut mins = Vec::with_capacity(len);
    let mut maxs = Vec::with_capacity(len);
    for i in 0..len {
        mins.push(mins_a[i].min(mins_b[i]));
        maxs.push(maxs_a[i].max(maxs_b[i]));
    }
    (mins, maxs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_merge_program_is_ir_not_target_text() {
        let program = interval_merge_program("amin", "amax", "bmin", "bmax", "omin", "omax", 16);
        let dump = format!("{program:#?}");
        assert!(dump.contains("Min"));
        assert!(dump.contains("Max"));
        assert!(!dump.contains("subgroupMin"));
        assert!(!dump.contains("vec2<u32>"));
    }

    #[test]
    fn cpu_interval_merge_is_conservative() {
        let (mins, maxs) = cpu_interval_merge(&[10, 0, 7], &[20, 3, 9], &[4, 2, 8], &[18, 5, 12]);
        assert_eq!(mins, vec![4, 0, 7]);
        assert_eq!(maxs, vec![20, 5, 12]);
    }
}
