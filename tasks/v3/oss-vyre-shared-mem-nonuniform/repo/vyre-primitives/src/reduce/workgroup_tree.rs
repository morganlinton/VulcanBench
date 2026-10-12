//! Workgroup-local tree reductions over scratch buffers.
//!
//! These helpers are Tier 2.5 LEGO blocks for higher-level library ops that
//! already stage one partial value per lane into workgroup memory. They emit
//! child `Region`s so composition audits and traces show the shared reduction
//! instead of treating every math/NN op as a hand-rolled loop.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for an f32 workgroup sum over a scratch buffer.
pub const SUM_F32_OP_ID: &str = "vyre-primitives::reduce::workgroup_sum_f32";
/// Canonical op id for a u32 workgroup sum over a scratch buffer.
pub const SUM_U32_OP_ID: &str = "vyre-primitives::reduce::workgroup_sum_u32";
/// Canonical op id for an f32 workgroup maximum over a scratch buffer.
pub const MAX_F32_OP_ID: &str = "vyre-primitives::reduce::workgroup_max_f32";
/// Canonical op id for a u32 workgroup maximum over a scratch buffer.
pub const MAX_U32_OP_ID: &str = "vyre-primitives::reduce::workgroup_max_u32";
/// Canonical op id for an f32 workgroup minimum over a scratch buffer.
pub const MIN_F32_OP_ID: &str = "vyre-primitives::reduce::workgroup_min_f32";
/// Canonical op id for a u32 workgroup minimum over a scratch buffer.
pub const MIN_U32_OP_ID: &str = "vyre-primitives::reduce::workgroup_min_u32";

/// Scope for a workgroup-local reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkgroupReductionScope {
    /// Every dispatched workgroup reduces its own scratch buffer.
    EveryWorkgroup,
    /// Only workgroup `x == 0` participates in the reduction.
    FirstWorkgroup,
}

impl WorkgroupReductionScope {
    fn lane_guard(self, lane_expr: Expr) -> Expr {
        match self {
            Self::EveryWorkgroup => lane_expr,
            Self::FirstWorkgroup => Expr::and(
                Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
                lane_expr,
            ),
        }
    }
}

/// Emit a child region that sums f32 lane partials in `scratch`.
#[must_use]
pub fn sum_f32_child(
    parent_op_id: &str,
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
) -> Node {
    child_region(SUM_F32_OP_ID, parent_op_id, sum_body(tile, scratch, scope))
}

/// Emit a child region that sums u32 lane partials in `scratch`.
#[must_use]
pub fn sum_u32_child(
    parent_op_id: &str,
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
) -> Node {
    child_region(SUM_U32_OP_ID, parent_op_id, sum_body(tile, scratch, scope))
}

/// Emit a child region that maximizes f32 lane partials in `scratch`.
#[must_use]
pub fn max_f32_child(
    parent_op_id: &str,
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
) -> Node {
    child_region(MAX_F32_OP_ID, parent_op_id, max_body(tile, scratch, scope))
}

/// Emit a child region that maximizes u32 lane partials in `scratch`.
#[must_use]
pub fn max_u32_child(
    parent_op_id: &str,
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
) -> Node {
    child_region(MAX_U32_OP_ID, parent_op_id, max_body(tile, scratch, scope))
}

/// Emit a child region that minimizes f32 lane partials in `scratch`.
#[must_use]
pub fn min_f32_child(
    parent_op_id: &str,
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
) -> Node {
    child_region(MIN_F32_OP_ID, parent_op_id, min_body(tile, scratch, scope))
}

/// Emit a child region that minimizes u32 lane partials in `scratch`.
#[must_use]
pub fn min_u32_child(
    parent_op_id: &str,
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
) -> Node {
    child_region(MIN_U32_OP_ID, parent_op_id, min_body(tile, scratch, scope))
}

/// Build a standalone f32 workgroup sum Program.
#[must_use]
pub fn workgroup_sum_f32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    reduction_program(
        SUM_F32_OP_ID,
        values,
        out,
        count,
        tile,
        DataType::F32,
        Expr::f32(0.0),
        Expr::add,
        |tile, scratch| sum_body(tile, scratch, WorkgroupReductionScope::FirstWorkgroup),
    )
}

/// Build a standalone u32 workgroup sum Program.
#[must_use]
pub fn workgroup_sum_u32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    reduction_program(
        SUM_U32_OP_ID,
        values,
        out,
        count,
        tile,
        DataType::U32,
        Expr::u32(0),
        Expr::add,
        |tile, scratch| sum_body(tile, scratch, WorkgroupReductionScope::FirstWorkgroup),
    )
}

/// Build a standalone f32 workgroup maximum Program.
#[must_use]
pub fn workgroup_max_f32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    reduction_program(
        MAX_F32_OP_ID,
        values,
        out,
        count,
        tile,
        DataType::F32,
        Expr::f32(f32::MIN),
        Expr::max,
        |tile, scratch| max_body(tile, scratch, WorkgroupReductionScope::FirstWorkgroup),
    )
}

/// Build a standalone u32 workgroup maximum Program.
///
/// The u32 twin of [`workgroup_max_f32`], closing the
/// sum-has-both-types / max-has-only-f32 asymmetry. `0` (`u32::MIN`) is the
/// neutral for an unsigned max, and the subgroup-first lowering already
/// recognizes the `workgroup_max_` prefix with a u32 value type, so this gets
/// the fast warp-reduction path on subgroup-capable backends for free.
#[must_use]
pub fn workgroup_max_u32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    reduction_program(
        MAX_U32_OP_ID,
        values,
        out,
        count,
        tile,
        DataType::U32,
        Expr::u32(u32::MIN),
        Expr::max,
        |tile, scratch| max_body(tile, scratch, WorkgroupReductionScope::FirstWorkgroup),
    )
}

/// Build a standalone f32 workgroup minimum Program.
///
/// `f32::MAX` is the neutral for a minimum (any real value is smaller). The
/// subgroup-first lowering recognizes the `workgroup_min_` prefix, so this gets
/// the native warp `subgroupMin` / `redux.sync.min` path on capable backends.
#[must_use]
pub fn workgroup_min_f32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    reduction_program(
        MIN_F32_OP_ID,
        values,
        out,
        count,
        tile,
        DataType::F32,
        Expr::f32(f32::MAX),
        Expr::min,
        |tile, scratch| min_body(tile, scratch, WorkgroupReductionScope::FirstWorkgroup),
    )
}

/// Build a standalone u32 workgroup minimum Program.
///
/// `u32::MAX` is the neutral for an unsigned minimum.
#[must_use]
pub fn workgroup_min_u32(values: &str, out: &str, count: u32, tile: u32) -> Program {
    reduction_program(
        MIN_U32_OP_ID,
        values,
        out,
        count,
        tile,
        DataType::U32,
        Expr::u32(u32::MAX),
        Expr::min,
        |tile, scratch| min_body(tile, scratch, WorkgroupReductionScope::FirstWorkgroup),
    )
}

#[allow(clippy::too_many_arguments)]
fn reduction_program<F, R>(
    op_id: &'static str,
    values: &str,
    out: &str,
    count: u32,
    tile: u32,
    dtype: DataType,
    init: Expr,
    accumulate: F,
    reduce: R,
) -> Program
where
    F: Fn(Expr, Expr) -> Expr,
    R: Fn(u32, &'static str) -> Vec<Node>,
{
    let tile = tile.max(1);
    let chunks = count.div_ceil(tile);
    let scratch = "__workgroup_reduce_scratch";
    let local = Expr::var("local");
    let idx = Expr::var("idx");
    let mut body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        Node::if_then(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("acc", init),
                Node::loop_for(
                    "chunk",
                    Expr::u32(0),
                    Expr::u32(chunks),
                    vec![
                        Node::let_bind(
                            "idx",
                            Expr::add(
                                Expr::mul(Expr::var("chunk"), Expr::u32(tile)),
                                local.clone(),
                            ),
                        ),
                        Node::if_then(
                            Expr::lt(idx.clone(), Expr::u32(count)),
                            vec![Node::assign(
                                "acc",
                                accumulate(Expr::var("acc"), Expr::load(values, idx.clone())),
                            )],
                        ),
                    ],
                ),
                Node::store(scratch, local.clone(), Expr::var("acc")),
            ],
        ),
        Node::barrier(),
    ];
    body.extend(reduce(tile, scratch));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            Expr::eq(local, Expr::u32(0)),
        ),
        vec![Node::store(
            out,
            Expr::u32(0),
            Expr::load(scratch, Expr::u32(0)),
        )],
    ));
    Program::wrapped(
        vec![
            BufferDecl::storage(values, 0, BufferAccess::ReadOnly, dtype.clone()).with_count(count),
            BufferDecl::workgroup(scratch, tile, dtype.clone()),
            BufferDecl::output(out, 1, dtype).with_count(1),
        ],
        [tile, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

fn child_region(generator: &'static str, parent_op_id: &str, body: Vec<Node>) -> Node {
    Node::Region {
        generator: Ident::from(generator),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(body),
    }
}

fn sum_body(tile: u32, scratch: &'static str, scope: WorkgroupReductionScope) -> Vec<Node> {
    tree_body(tile, scratch, scope, Expr::add)
}

fn max_body(tile: u32, scratch: &'static str, scope: WorkgroupReductionScope) -> Vec<Node> {
    tree_body(tile, scratch, scope, Expr::max)
}

fn min_body(tile: u32, scratch: &'static str, scope: WorkgroupReductionScope) -> Vec<Node> {
    tree_body(tile, scratch, scope, Expr::min)
}

fn tree_body<F>(
    tile: u32,
    scratch: &'static str,
    scope: WorkgroupReductionScope,
    combine: F,
) -> Vec<Node>
where
    F: Fn(Expr, Expr) -> Expr,
{
    let mut nodes = Vec::new();
    let mut stride = tile.next_power_of_two() / 2;
    while stride > 0 {
        let lhs = Expr::load(scratch, Expr::var("local"));
        let rhs_index = Expr::add(Expr::var("local"), Expr::u32(stride));
        let rhs = Expr::load(scratch, rhs_index.clone());
        nodes.push(Node::if_then(
            scope.lane_guard(Expr::lt(Expr::var("local"), Expr::u32(stride))),
            vec![Node::if_then(
                Expr::lt(rhs_index, Expr::u32(tile)),
                vec![Node::Store {
                    buffer: scratch.into(),
                    index: Expr::var("local"),
                    value: combine(lhs, rhs),
                }],
            )],
        ));
        nodes.push(Node::barrier());
        stride /= 2;
    }
    nodes
}

#[cfg(feature = "inventory-registry")]
fn fixture_f32(values: &[f32]) -> Vec<u8> {
    crate::wire::pack_f32_slice(values)
}

#[cfg(feature = "inventory-registry")]
fn fixture_u32(values: &[u32]) -> Vec<u8> {
    crate::wire::pack_u32_slice(values)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        SUM_F32_OP_ID,
        || workgroup_sum_f32("values", "out", 4, 4),
        Some(|| vec![vec![
            fixture_f32(&[1.25, -2.0, 5.5, 3.25]),
            fixture_f32(&[0.0]),
        ]]),
        Some(|| vec![vec![fixture_f32(&[8.0])]]),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        SUM_U32_OP_ID,
        || workgroup_sum_u32("values", "out", 4, 4),
        Some(|| vec![vec![
            fixture_u32(&[1, 2, 3, 4]),
            fixture_u32(&[0]),
        ]]),
        Some(|| vec![vec![fixture_u32(&[10])]]),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        MAX_F32_OP_ID,
        || workgroup_max_f32("values", "out", 4, 4),
        Some(|| vec![vec![
            fixture_f32(&[-3.0, 9.5, 4.0, 1.25]),
            fixture_f32(&[0.0]),
        ]]),
        Some(|| vec![vec![fixture_f32(&[9.5])]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    #[test]
    fn child_region_names_parent_and_primitive() {
        let node = sum_f32_child(
            "vyre-libs::math::reduce_mean",
            256,
            "scratch",
            WorkgroupReductionScope::FirstWorkgroup,
        );
        let Node::Region {
            generator,
            source_region,
            body,
        } = node
        else {
            panic!("Fix: workgroup tree helper must emit a child Region.");
        };
        assert_eq!(generator.as_str(), SUM_F32_OP_ID);
        assert_eq!(
            source_region
                .expect("Fix: child Region must name parent.")
                .name,
            "vyre-libs::math::reduce_mean"
        );
        assert!(!body.is_empty());
    }

    #[test]
    fn standalone_sum_f32_matches_reference_arithmetic() {
        let values = [1.25_f32, -2.0, 5.5, 3.25, 8.0];
        let program = workgroup_sum_f32("values", "out", values.len() as u32, 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_f32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: workgroup_sum_f32 must execute in the reference interpreter.");
        assert_eq!(
            crate::wire::decode_f32_le_bytes_all(&outputs[0].to_bytes())[0],
            values.iter().copied().sum::<f32>()
        );
    }

    #[test]
    fn standalone_sum_u32_matches_reference_arithmetic() {
        let values = [1_u32, 2, 3, 4, 5, 6, 7];
        let program = workgroup_sum_u32("values", "out", values.len() as u32, 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_u32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<u32>()]),
            ],
        )
        .expect("Fix: workgroup_sum_u32 must execute in the reference interpreter.");
        assert_eq!(
            crate::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())[0],
            values.iter().copied().sum::<u32>()
        );
    }

    #[test]
    fn standalone_max_f32_matches_reference_arithmetic() {
        let values = [-3.0_f32, 9.5, 4.0, 1.25, 8.75];
        let program = workgroup_max_f32("values", "out", values.len() as u32, 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_f32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: workgroup_max_f32 must execute in the reference interpreter.");
        assert_eq!(
            crate::wire::decode_f32_le_bytes_all(&outputs[0].to_bytes())[0],
            9.5
        );
    }

    #[test]
    fn standalone_max_u32_matches_reference_arithmetic() {
        // Max (42) is at index 3, not 0, so a broken reduction that kept the
        // first lane or the `0` identity would be caught.
        let values = [3_u32, 17, 5, 42, 8, 1];
        let program = workgroup_max_u32("values", "out", values.len() as u32, 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_u32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<u32>()]),
            ],
        )
        .expect("Fix: workgroup_max_u32 must execute in the reference interpreter.");
        assert_eq!(
            crate::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())[0],
            values.iter().copied().max().expect("non-empty"),
            "workgroup_max_u32 must compute the unsigned max (42)"
        );
    }

    #[test]
    fn standalone_min_f32_matches_reference_arithmetic() {
        // Min (-2.5) is not at index 0; an f32::MAX-identity or kept-first bug fails.
        let values = [3.0_f32, 9.5, -2.5, 1.25, 8.0];
        let program = workgroup_min_f32("values", "out", values.len() as u32, 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_f32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: workgroup_min_f32 must execute in the reference interpreter.");
        assert_eq!(
            crate::wire::decode_f32_le_bytes_all(&outputs[0].to_bytes())[0],
            values.iter().copied().fold(f32::INFINITY, f32::min),
            "workgroup_min_f32 must compute the min (-2.5)"
        );
    }

    #[test]
    fn standalone_min_u32_matches_reference_arithmetic() {
        let values = [17_u32, 3, 42, 8, 25];
        let program = workgroup_min_u32("values", "out", values.len() as u32, 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_u32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<u32>()]),
            ],
        )
        .expect("Fix: workgroup_min_u32 must execute in the reference interpreter.");
        assert_eq!(
            crate::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())[0],
            values.iter().copied().min().expect("non-empty"),
            "workgroup_min_u32 must compute the unsigned min (3)"
        );
    }

    #[test]
    fn non_power_of_two_tile_reductions_match_reference_arithmetic() {
        let values = [4.0_f32, -7.0, 2.5, 9.0, 1.0, 3.25, -2.0];
        let sum_program = workgroup_sum_f32("values", "out", values.len() as u32, 3);
        let sum_outputs = vyre_reference::reference_eval(
            &sum_program,
            &[
                Value::from(crate::wire::pack_f32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: workgroup_sum_f32 must support non-power-of-two tiles.");
        assert_eq!(
            crate::wire::decode_f32_le_bytes_all(&sum_outputs[0].to_bytes())[0],
            values.iter().copied().sum::<f32>()
        );

        let max_program = workgroup_max_f32("values", "out", values.len() as u32, 3);
        let max_outputs = vyre_reference::reference_eval(
            &max_program,
            &[
                Value::from(crate::wire::pack_f32_slice(&values)),
                Value::from(vec![0_u8; core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: workgroup_max_f32 must support non-power-of-two tiles.");
        assert_eq!(
            crate::wire::decode_f32_le_bytes_all(&max_outputs[0].to_bytes())[0],
            9.0
        );
    }
}
