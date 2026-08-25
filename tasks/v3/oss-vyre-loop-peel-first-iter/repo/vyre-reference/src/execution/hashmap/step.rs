//! Round-robin node stepping and expression-adjacent execution helpers.
mod node_step {
    include!("node_step.rs");
}
pub(crate) use node_step::{eval_call, step_loop_frame, step_nodes_frame};

#[cfg(feature = "subgroup-ops")]
use super::state::HashmapInvocationSnapshot;
use super::{memory::HashmapMemory, state::HashmapInvocation};
use crate::{value::Value, workgroup::Frame};
use vyre::Error;

pub(crate) fn step_round_robin(
    memory: &mut HashmapMemory,
    invocations: &mut [HashmapInvocation<'_>],
    #[cfg(feature = "subgroup-ops")] uses_subgroup_ops: bool,
) -> Result<bool, Error> {
    let mut made_progress = false;
    #[cfg(feature = "subgroup-ops")]
    let snapshots = if uses_subgroup_ops {
        capture_invocation_snapshots(invocations)
    } else {
        Vec::new()
    };
    for index in 0..invocations.len() {
        if invocations[index].done() || invocations[index].waiting_at_barrier {
            continue;
        }
        step(
            index,
            memory,
            invocations,
            #[cfg(feature = "subgroup-ops")]
            &snapshots,
        )?;
        made_progress = true;
    }
    Ok(made_progress)
}

fn step(
    index: usize,
    memory: &mut HashmapMemory,
    invocations: &mut [HashmapInvocation<'_>],
    #[cfg(feature = "subgroup-ops")] snapshots: &[HashmapInvocationSnapshot],
) -> Result<(), Error> {
    let invocation = &mut invocations[index];
    if invocation.done() || invocation.waiting_at_barrier {
        return Ok(());
    }
    loop {
        let Some(frame) = invocation.frames.pop() else {
            return Ok(());
        };
        match frame {
            Frame::Nodes {
                nodes,
                index,
                scoped,
            } => {
                if step_nodes_frame(
                    invocation,
                    memory,
                    nodes,
                    index,
                    scoped,
                    #[cfg(feature = "subgroup-ops")]
                    snapshots,
                )? {
                    return Ok(());
                }
            }
            Frame::Loop {
                var,
                next,
                to,
                body,
            } => {
                step_loop_frame(invocation, var, next, to, body)?;
                return Ok(());
            }
        }
    }
}

pub(crate) fn axis_value(values: [u32; 3], axis: u8) -> Result<Value, Error> {
    values
        .get(axis as usize)
        .copied()
        .map(Value::U32)
        .ok_or_else(|| {
            Error::interp(format!(
                "invocation/workgroup ID axis {axis} out of range. Fix: use 0, 1, or 2."
            ))
        })
}

pub(crate) fn eval_to_index(
    expr: &vyre::ir::Expr,
    label: &str,
    invocation: &mut HashmapInvocation<'_>,
    memory: &mut HashmapMemory,
    #[cfg(feature = "subgroup-ops")] snapshots: &[HashmapInvocationSnapshot],
) -> Result<u32, Error> {
    super::eval_expr(
        expr,
        invocation,
        memory,
        #[cfg(feature = "subgroup-ops")]
        snapshots,
    )?
    .try_as_u32()
    .ok_or_else(|| {
        Error::interp(format!(
            "{label} cannot be represented as u32. Fix: use a non-negative scalar index within u32."
        ))
    })
}

#[cfg(feature = "subgroup-ops")]
pub(crate) fn eval_expr_snapshot(
    expr: &vyre::ir::Expr,
    snapshot: &HashmapInvocationSnapshot,
    snapshots: &[HashmapInvocationSnapshot],
    memory: &HashmapMemory,
) -> Result<Value, Error> {
    let empty_entry: &[vyre::ir::Node] = &[];
    let mut invocation =
        HashmapInvocation::new(snapshot.ids, snapshot.linear_local_index, empty_entry);
    invocation.locals.locals = snapshot.locals.locals.clone();
    let mut snapshot_memory = HashmapMemory {
        storage: memory.storage.clone(),
        workgroup: memory.workgroup.clone(),
    };
    super::eval_expr(expr, &mut invocation, &mut snapshot_memory, snapshots)
}

#[cfg(feature = "subgroup-ops")]
fn capture_invocation_snapshots(
    invocations: &[HashmapInvocation<'_>],
) -> Vec<HashmapInvocationSnapshot> {
    invocations
        .iter()
        .map(|invocation| HashmapInvocationSnapshot {
            ids: invocation.ids,
            linear_local_index: invocation.linear_local_index,
            locals: invocation.locals.snapshot(),
        })
        .collect()
}

#[cfg(all(test, feature = "subgroup-ops"))]
mod tests {
    use super::capture_invocation_snapshots;
    use crate::execution::hashmap::state::HashmapInvocation;
    use crate::value::Value;
    use crate::workgroup::InvocationIds;
    use std::sync::Arc;
    use vyre::ir::Node;

    #[test]
    fn subgroup_snapshots_share_persistent_local_maps() {
        let entry: &[Node] = &[];
        let mut invocation = HashmapInvocation::new(InvocationIds::ZERO, 0, entry);
        for index in 0..256 {
            invocation
                .locals
                .bind(
                    &format!("lane_value_{index}"),
                    Value::Bytes(Arc::from(vec![index as u8; 4096])),
                )
                .expect("Fix: generated locals must bind once");
        }
        invocation.locals.push_scope();
        invocation
            .locals
            .bind("scoped", Value::U32(7))
            .expect("Fix: scoped local must bind once");

        let invocations = [invocation];
        let snapshots = capture_invocation_snapshots(&invocations);

        assert!(
            snapshots[0].locals.locals.ptr_eq(&invocations[0].locals.locals),
            "Fix: subgroup snapshots must clone the persistent locals root instead of rebuilding or deep-cloning values"
        );
        assert_eq!(
            snapshots[0].locals.local("scoped"),
            Some(Value::U32(7)),
            "Fix: subgroup snapshots must retain active locals without copying scope stacks"
        );
    }
}
