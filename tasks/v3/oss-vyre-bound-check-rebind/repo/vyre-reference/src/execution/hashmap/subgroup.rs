//! Subgroup collective semantics for the HashMap interpreter.
//!
//! These helpers operate on immutable invocation snapshots so collectives
//! observe a stable workgroup lane view.

#[cfg(feature = "subgroup-ops")]
use super::{
    memory::HashmapMemory,
    state::{HashmapInvocation, HashmapInvocationSnapshot},
    step::eval_expr_snapshot,
};
#[cfg(feature = "subgroup-ops")]
use crate::{subgroup::SubgroupSimulator, value::Value};
#[cfg(feature = "subgroup-ops")]
use smallvec::SmallVec;
#[cfg(feature = "subgroup-ops")]
use std::sync::OnceLock;
#[cfg(feature = "subgroup-ops")]
use vyre::ir::{Expr, SubgroupReduceOp};
#[cfg(feature = "subgroup-ops")]
use vyre::Error;

#[cfg(feature = "subgroup-ops")]
pub(crate) fn subgroup_simulator() -> &'static SubgroupSimulator {
    static SIMULATOR: OnceLock<SubgroupSimulator> = OnceLock::new();
    SIMULATOR.get_or_init(SubgroupSimulator::default)
}

#[cfg(feature = "subgroup-ops")]
pub(crate) fn subgroup_slice(
    snapshots: &[HashmapInvocationSnapshot],
    linear_local_index: u32,
) -> &[HashmapInvocationSnapshot] {
    let simulator = subgroup_simulator();
    let lane_index = linear_local_index as usize;
    let (start, end) = simulator.subgroup_bounds(snapshots.len(), lane_index);
    &snapshots[start..end]
}

#[cfg(feature = "subgroup-ops")]
pub(crate) fn eval_subgroup_ballot(
    cond: &Expr,
    invocation: &HashmapInvocation<'_>,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
) -> Result<Value, Error> {
    let mask = collect_lane_bools(cond, invocation.linear_local_index, snapshots, memory)?;
    Ok(Value::U32(subgroup_simulator().ballot_slice(&mask)))
}

#[cfg(feature = "subgroup-ops")]
pub(crate) fn eval_subgroup_shuffle(
    value: &Expr,
    lane: &Expr,
    invocation: &HashmapInvocation<'_>,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
) -> Result<Value, Error> {
    let values = collect_lane_values(value, invocation.linear_local_index, snapshots, memory)?;
    let src_lanes = collect_lane_u32s(
        lane,
        invocation.linear_local_index,
        snapshots,
        memory,
        "subgroup_shuffle lane index is not a u32. Fix: use a scalar u32 lane argument.",
    )?;
    let local_offset = (invocation.linear_local_index as usize) % subgroup_simulator().width();
    let src_lane = src_lanes.get(local_offset).copied().unwrap_or(u32::MAX) as usize;
    if values.iter().all(|value| matches!(value, Value::U32(_))) {
        let lanes = values
            .iter()
            .filter_map(Value::try_as_u32)
            .collect::<SmallVec<[u32; 32]>>();
        let shuffled = subgroup_simulator().shuffle(&lanes, &src_lanes);
        return Ok(Value::U32(shuffled.get(local_offset).copied().unwrap_or(0)));
    }
    if values.iter().all(|value| matches!(value, Value::Float(_))) {
        return match values.get(src_lane) {
            Some(Value::Float(value)) => Ok(Value::Float(*value)),
            _ => Ok(Value::Float(0.0)),
        };
    }
    Err(Error::interp(
        "subgroup_shuffle lanes have mixed or unsupported value types. Fix: cast every lane value to the same primitive u32 or f32 type before the subgroup collective.",
    ))
}

#[cfg(feature = "subgroup-ops")]
pub(crate) fn eval_subgroup_reduce(
    op: SubgroupReduceOp,
    value: &Expr,
    invocation: &HashmapInvocation<'_>,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
) -> Result<Value, Error> {
    let values = collect_lane_values(value, invocation.linear_local_index, snapshots, memory)?;
    let width = subgroup_simulator().width();
    if values.iter().all(|value| matches!(value, Value::U32(_))) {
        let lanes = values.iter().filter_map(Value::try_as_u32).take(width);
        // SubgroupReduceOp::reduce_u32 is the single source of truth for the
        // operator's integer semantics (shared with const folding / spec).
        return Ok(Value::U32(op.reduce_u32(lanes)));
    }
    if values.iter().all(|value| matches!(value, Value::Float(_))) {
        let Some(identity) = op.f32_identity() else {
            return Err(Error::interp(
                "subgroup bitwise reduction (and/or/xor) requires integer lanes, not f32. Fix: use an integer value or a non-bitwise reduction operator.",
            ));
        };
        let mut acc = identity;
        for value in values.iter().take(width) {
            let Value::Float(lane) = value else { continue };
            let Some(combined) = op.combine_f32(acc, *lane as f32) else {
                return Err(Error::interp(
                    "subgroup reduction operator is not defined on f32 lanes. Fix: extend SubgroupReduceOp::combine_f32 or use integer lanes.",
                ));
            };
            acc = crate::execution::typed_ops::canonical_f32(combined);
        }
        return Ok(Value::Float(f64::from(acc)));
    }
    Err(Error::interp(
        "subgroup reduce lanes have mixed or unsupported value types. Fix: cast every lane value to the same primitive u32 or f32 type before the subgroup collective.",
    ))
}

#[cfg(feature = "subgroup-ops")]
fn collect_lane_bools(
    expr: &Expr,
    linear_local_index: u32,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
) -> Result<SmallVec<[bool; 32]>, Error> {
    subgroup_slice(snapshots, linear_local_index)
        .iter()
        .map(|lane| eval_expr_snapshot(expr, lane, snapshots, memory).map(|value| value.truthy()))
        .collect()
}

#[cfg(feature = "subgroup-ops")]
fn collect_lane_u32s(
    expr: &Expr,
    linear_local_index: u32,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
    error: &'static str,
) -> Result<SmallVec<[u32; 32]>, Error> {
    subgroup_slice(snapshots, linear_local_index)
        .iter()
        .map(|lane| {
            eval_expr_snapshot(expr, lane, snapshots, memory)?
                .try_as_u32()
                .ok_or_else(|| Error::interp(error))
        })
        .collect()
}

#[cfg(feature = "subgroup-ops")]
fn collect_lane_values(
    expr: &Expr,
    linear_local_index: u32,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
) -> Result<SmallVec<[Value; 32]>, Error> {
    subgroup_slice(snapshots, linear_local_index)
        .iter()
        .map(|lane| eval_expr_snapshot(expr, lane, snapshots, memory))
        .collect()
}

#[cfg(all(test, feature = "subgroup-ops"))]
mod tests {
    use super::*;
    use crate::workgroup::InvocationIds;
    use rustc_hash::FxHashMap;
    use vyre::ir::Node;

    fn snapshot_lane(index: u32, value: Value, source_lane: u32) -> HashmapInvocationSnapshot {
        let entry: &[Node] = &[];
        let mut invocation = HashmapInvocation::new(InvocationIds::ZERO, index, entry);
        assert!(
            invocation.locals.bind("lane_value", value).is_ok(),
            "Fix: lane_value binding must be unique."
        );
        assert!(
            invocation
                .locals
                .bind("source_lane", Value::U32(source_lane))
                .is_ok(),
            "Fix: source_lane binding must be unique."
        );
        HashmapInvocationSnapshot {
            ids: invocation.ids,
            linear_local_index: invocation.linear_local_index,
            locals: invocation.locals.snapshot(),
        }
    }

    #[test]
    fn f32_shuffle_preserves_selected_lane_value() {
        let snapshots = vec![
            snapshot_lane(0, Value::Float(1.25), 2),
            snapshot_lane(1, Value::Float(2.5), 0),
            snapshot_lane(2, Value::Float(3.75), 1),
        ];
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_shuffle(
            &Expr::var("lane_value"),
            &Expr::var("source_lane"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("Fix: f32 subgroup shuffle must evaluate.");

        assert_eq!(value, Value::Float(3.75));
    }

    #[test]
    fn f32_shuffle_zeroes_out_of_range_lane() {
        let snapshots = vec![
            snapshot_lane(0, Value::Float(1.25), 9),
            snapshot_lane(1, Value::Float(2.5), 0),
        ];
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_shuffle(
            &Expr::var("lane_value"),
            &Expr::var("source_lane"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("Fix: f32 subgroup shuffle must evaluate.");

        assert_eq!(value, Value::Float(0.0));
    }

    fn reduce_snapshots(values: &[Value]) -> Vec<HashmapInvocationSnapshot> {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| snapshot_lane(index as u32, value.clone(), 0))
            .collect()
    }

    #[test]
    fn u32_max_reduces_across_lanes() {
        let snapshots = reduce_snapshots(&[Value::U32(3), Value::U32(9), Value::U32(1)]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_reduce(
            SubgroupReduceOp::Max,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("u32 subgroup max must evaluate");

        assert_eq!(value, Value::U32(9));
    }

    #[test]
    fn u32_xor_reduces_across_lanes() {
        let snapshots = reduce_snapshots(&[Value::U32(0b1100), Value::U32(0b1010)]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_reduce(
            SubgroupReduceOp::Xor,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("u32 subgroup xor must evaluate");

        assert_eq!(value, Value::U32(0b0110));
    }

    #[test]
    fn u32_mul_reduces_across_lanes() {
        // Product is the differential ground truth for the PTX integer
        // shfl-butterfly path (which has no `redux.sync`): 2*3*5*7 = 210.
        let snapshots =
            reduce_snapshots(&[Value::U32(2), Value::U32(3), Value::U32(5), Value::U32(7)]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_reduce(
            SubgroupReduceOp::Mul,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("u32 subgroup mul must evaluate");

        assert_eq!(value, Value::U32(210));
    }

    #[test]
    fn f32_max_uses_neg_inf_identity_for_all_negative_lanes() {
        // The -inf identity is load-bearing: a 0 identity would wrongly win
        // when every lane is negative.
        let snapshots = reduce_snapshots(&[Value::Float(-5.0), Value::Float(-2.0)]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_reduce(
            SubgroupReduceOp::Max,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("f32 subgroup max must evaluate");

        assert_eq!(value, Value::Float(-2.0));
    }

    #[test]
    fn f32_bitwise_reduce_fails_loud() {
        let snapshots = reduce_snapshots(&[Value::Float(1.0)]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let error = eval_subgroup_reduce(
            SubgroupReduceOp::And,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect_err("bitwise reduction on f32 must fail loud, not silently degrade");

        assert!(
            format!("{error}").contains("bitwise"),
            "error must name the bitwise/f32 mismatch: {error}"
        );
    }

    #[test]
    fn f32_mul_reduces_across_lanes() {
        // Spec ground truth for the f32 shfl-butterfly `mul.f32` combine. The
        // factors are exact dyadic values (2*3*0.5*4 = 12.0) matching the live
        // GPU parity test `cuda_subgroup_mul_f32_*`, so oracle and hardware form
        // a true differential pair on the same value.
        let snapshots = reduce_snapshots(&[
            Value::Float(2.0),
            Value::Float(3.0),
            Value::Float(0.5),
            Value::Float(4.0),
        ]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_reduce(
            SubgroupReduceOp::Mul,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("f32 subgroup mul must evaluate");

        assert_eq!(value, Value::Float(12.0));
    }

    #[test]
    fn f32_add_reduces_across_lanes() {
        // The 0.0 additive identity fold: 1.0 + 2.5 + 3.5 = 7.0.
        let snapshots =
            reduce_snapshots(&[Value::Float(1.0), Value::Float(2.5), Value::Float(3.5)]);
        let entry: &[Node] = &[];
        let invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        let memory = HashmapMemory::new(FxHashMap::default());

        let value = eval_subgroup_reduce(
            SubgroupReduceOp::Add,
            &Expr::var("lane_value"),
            &invocation,
            &snapshots,
            &memory,
        )
        .expect("f32 subgroup add must evaluate");

        assert_eq!(value, Value::Float(7.0));
    }
}
