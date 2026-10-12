//! Statement executor that gives the parity engine a pure-Rust ground truth
//! for every `Node` variant.
//!
//! This module simulates the exact control-flow, memory, and barrier behavior
//! that a correct GPU backend must produce. Any divergence in `If`, `Loop`,
//! `Barrier`, or `Store` semantics is caught by the conform gate as a concrete
//! counterexample.

use vyre::ir::{Expr, Node, Program};

use crate::{
    execution::expr as eval_expr,
    execution::node_tree::{contains_barrier, node_id},
    oob,
    workgroup::{AsyncTransfer, Frame, Invocation, Memory},
};
use vyre::Error;

/// Execute one scheduling step for an invocation.
///
/// # Errors
///
/// Returns [`Error::Interp`] for uniform-control-flow violations,
/// out-of-bounds stores, malformed loops, or expression evaluation failures.
pub fn step<'a>(
    invocation: &mut Invocation<'a>,
    memory: &mut Memory,
    program: &'a Program,
) -> Result<(), vyre::Error> {
    if invocation.done() || invocation.waiting_at_barrier {
        return Ok(());
    }

    loop {
        let Some(frame) = invocation.frames_mut().pop() else {
            return Ok(());
        };
        match frame {
            Frame::Nodes {
                nodes,
                index,
                scoped,
            } => {
                if step_nodes_frame(invocation, memory, program, nodes, index, scoped)? {
                    return Ok(());
                }
            }
            Frame::Loop {
                var,
                next,
                to,
                body,
            } => step_loop_frame(invocation, var, next, to, body)?,
        }
    }
}

fn step_nodes_frame<'a>(
    invocation: &mut Invocation<'a>,
    memory: &mut Memory,
    program: &'a Program,
    nodes: &'a [Node],
    index: usize,
    scoped: bool,
) -> Result<bool, vyre::Error> {
    if index >= nodes.len() {
        if scoped {
            invocation.pop_scope();
        }
        return Ok(false);
    }

    invocation.frames_mut().push(Frame::Nodes {
        nodes,
        index: index + 1,
        scoped,
    });
    execute_node(&nodes[index], invocation, memory, program)?;
    Ok(true)
}

fn step_loop_frame<'a>(
    invocation: &mut Invocation<'a>,
    var: &'a str,
    next: u32,
    to: u32,
    body: &'a [Node],
) -> Result<(), vyre::Error> {
    if next >= to {
        return Ok(());
    }
    invocation.frames_mut().push(Frame::Loop {
        var,
        next: next.wrapping_add(1),
        to,
        body,
    });
    invocation.push_scope();
    invocation.bind_loop_var(var, crate::value::Value::U32(next))?;
    invocation.frames_mut().push(Frame::Nodes {
        nodes: body,
        index: 0,
        scoped: true,
    });
    Ok(())
}

fn execute_node<'a>(
    node: &'a Node,
    invocation: &mut Invocation<'a>,
    memory: &mut Memory,
    program: &'a Program,
) -> Result<(), vyre::Error> {
    match node {
        Node::Let { name, value } => eval_let(name, value, invocation, memory, program),
        Node::Assign { name, value } => eval_assign(name, value, invocation, memory, program),
        Node::Store {
            buffer,
            index,
            value,
        } => eval_store(buffer, index, value, invocation, memory, program),
        Node::If {
            cond,
            then,
            otherwise,
        } => eval_if(cond, then, otherwise, node, invocation, memory, program),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => eval_loop(var, from, to, body, invocation, memory, program),
        Node::Return => eval_return(invocation),
        Node::Block(nodes) => eval_block(nodes, invocation),
        Node::Barrier { .. } => eval_barrier(invocation),
        Node::IndirectDispatch {
            count_buffer,
            count_offset,
        } => eval_indirect_dispatch(count_buffer, *count_offset, memory, program),
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => eval_async_load(
            AsyncLoadEval {
                source,
                destination,
                offset,
                size,
                tag,
            },
            invocation,
            memory,
            program,
        ),
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => eval_async_store(
            AsyncStoreEval {
                source,
                destination,
                offset,
                size,
                tag,
            },
            invocation,
            memory,
            program,
        ),
        Node::AsyncWait { tag } => eval_async_wait(tag, invocation, memory, program),
        Node::Trap { address, tag } => {
            let address = eval_expr::eval(address, invocation, memory, program)?
                .try_as_u32()
                .ok_or_else(|| {
                    Error::interp(format!(
                        "reference trap `{tag}` address is not a u32. Fix: pass a scalar u32 trap address."
                    ))
                })?;
            Err(vyre::Error::interp(format!(
                "reference dispatch trapped: address={address}, tag=`{tag}`. Fix: handle the trap condition or route this Program through a backend/runtime with replay support."
            )))
        }
        Node::Resume { tag } => Err(vyre::Error::interp(format!(
            "reference dispatch reached Resume `{tag}` without a replay runtime. Fix: lower Resume through a runtime-owned replay path before reference execution."
        ))),
        Node::AllReduce { buffer, group, .. } => Err(vyre::Error::interp(format!(
            "reference dispatch reached AllReduce on buffer `{buffer}` for group {}. Fix: run this Program on a distributed backend with collective support or lower the single-rank collective before reference execution.",
            group.as_u32()
        ))),
        Node::AllGather {
            input,
            output,
            group,
        } => Err(vyre::Error::interp(format!(
            "reference dispatch reached AllGather `{input}` -> `{output}` for group {}. Fix: run this Program on a distributed backend with collective support or lower the single-rank collective before reference execution.",
            group.as_u32()
        ))),
        Node::ReduceScatter {
            input,
            output,
            group,
            ..
        } => Err(vyre::Error::interp(format!(
            "reference dispatch reached ReduceScatter `{input}` -> `{output}` for group {}. Fix: run this Program on a distributed backend with collective support or lower the single-rank collective before reference execution.",
            group.as_u32()
        ))),
        Node::Broadcast {
            buffer,
            root,
            group,
        } => Err(vyre::Error::interp(format!(
            "reference dispatch reached Broadcast on buffer `{buffer}` from root {root} for group {}. Fix: run this Program on a distributed backend with collective support or lower the single-rank collective before reference execution.",
            group.as_u32()
        ))),
        Node::Region { body, .. } => eval_block(body, invocation),
        Node::Opaque(extension) => Err(vyre::Error::interp(format!(
            "reference interpreter does not support opaque node extension `{}`/`{}`. Fix: provide a reference evaluator for this NodeExtension or lower it to core Node variants before evaluation.",
            extension.extension_kind(),
            extension.debug_identity()
        ))),
        _ => Err(vyre::Error::interp(
            "reference interpreter encountered an unknown Node variant. Fix: update vyre-reference before executing this IR.",
        )),
    }
}

fn eval_let(
    name: &str,
    value: &Expr,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let value = eval_expr::eval(value, invocation, memory, program)?;
    invocation.bind(name, value)
}

fn eval_assign(
    name: &str,
    value: &Expr,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let value = eval_expr::eval(value, invocation, memory, program)?;
    invocation.assign(name, value)
}

fn eval_store(
    buffer: &str,
    index: &Expr,
    value: &Expr,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let index = eval_expr::eval(index, invocation, memory, program)?;
    let index = index
        .try_as_u32()
        .ok_or_else(|| Error::interp(format!(
                "store index {index:?} cannot be represented as u32. Fix: use a non-negative scalar index within u32."
        )))?;
    let value = eval_expr::eval(value, invocation, memory, program)?;
    let target = eval_expr::buffer_mut(memory, program, buffer)?;
    oob::store(target, index, &value);
    Ok(())
}

fn eval_indirect_dispatch(
    count_buffer: &str,
    count_offset: u64,
    memory: &Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    if count_offset % 4 != 0 {
        return Err(Error::interp(format!(
            "indirect dispatch offset {count_offset} is not 4-byte aligned. Fix: use a u32-aligned dispatch tuple."
        )));
    }
    let decl = program.buffer(count_buffer).ok_or_else(|| {
        Error::interp(format!(
            "indirect dispatch references unknown buffer `{count_buffer}`. Fix: declare the count buffer before execution."
        ))
    })?;
    let buffer = if decl.access() == vyre::ir::BufferAccess::Workgroup {
        memory.workgroup.get(count_buffer)
    } else {
        memory.storage.get(count_buffer)
    }
    .ok_or_else(|| {
        Error::interp(format!(
            "indirect dispatch buffer `{count_buffer}` is missing. Fix: initialize the count buffer before execution."
        ))
    })?;
    let required_end = count_offset.checked_add(12).ok_or_else(|| {
        Error::interp(
            "indirect dispatch byte range overflowed u64. Fix: shrink the count offset."
                .to_string(),
        )
    })?;
    let byte_len = buffer
        .bytes
        .read()
        .map_err(|_| {
            Error::interp(format!(
                "indirect dispatch buffer `{count_buffer}` lock is poisoned. Fix: rebuild the interpreter memory state before execution."
            ))
        })?
        .len();
    if u64::try_from(byte_len).unwrap_or(u64::MAX) < required_end {
        return Err(Error::interp(format!(
            "indirect dispatch buffer `{count_buffer}` is too short for a 3-word dispatch tuple at byte offset {count_offset}. Fix: provide 12 readable bytes starting at that offset."
        )));
    }
    Err(Error::interp(format!(
        "Node::IndirectDispatch cannot execute in the sequential reference interpreter because dynamic indirect dispatch requires runtime queue scheduling. Fix: run this program on a backend/runtime that supports indirect dispatch or lower `{count_buffer}` at byte offset {count_offset} to a static workgroup grid before reference execution."
    )))
}

struct AsyncLoadEval<'a> {
    source: &'a str,
    destination: &'a str,
    offset: &'a Expr,
    size: &'a Expr,
    tag: &'a str,
}

struct AsyncStoreEval<'a> {
    source: &'a str,
    destination: &'a str,
    offset: &'a Expr,
    size: &'a Expr,
    tag: &'a str,
}

fn eval_async_load(
    request: AsyncLoadEval<'_>,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let start = eval_byte_count(
        request.offset,
        "async load source offset",
        invocation,
        memory,
        program,
    )?;
    let byte_count = eval_byte_count(request.size, "async load size", invocation, memory, program)?;
    let payload = read_bytes(memory, program, request.source, start, byte_count)?;
    ensure_writable_buffer(memory, program, request.destination)?;
    invocation.begin_async(
        request.tag,
        AsyncTransfer::Copy {
            destination: request.destination.into(),
            start: 0,
            payload,
        },
    )
}

fn eval_async_store(
    request: AsyncStoreEval<'_>,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let start = eval_byte_count(
        request.offset,
        "async store destination offset",
        invocation,
        memory,
        program,
    )?;
    let byte_count = eval_byte_count(
        request.size,
        "async store size",
        invocation,
        memory,
        program,
    )?;
    let payload = read_bytes(memory, program, request.source, 0, byte_count)?;
    ensure_writable_buffer(memory, program, request.destination)?;
    invocation.begin_async(
        request.tag,
        AsyncTransfer::Copy {
            destination: request.destination.into(),
            start,
            payload,
        },
    )
}

fn eval_async_wait(
    tag: &str,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    apply_async_transfer(invocation.finish_async(tag)?, memory, program)
}

fn eval_byte_count(
    expr: &Expr,
    label: &str,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<usize, Error> {
    let value = eval_expr::eval(expr, invocation, memory, program)?;
    usize::try_from(value.try_as_u64().ok_or_else(|| {
        Error::interp(format!(
            "{label} cannot be represented as u64. Fix: use an in-range non-negative byte count."
        ))
    })?)
    .map_err(|_| {
        Error::interp(format!(
            "{label} exceeds host usize. Fix: reduce the async transfer span."
        ))
    })
}

fn read_bytes(
    memory: &Memory,
    program: &Program,
    source: &str,
    start: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Error> {
    let buffer = resolve_buffer(memory, program, source)?;
    let bytes = buffer
        .bytes
        .read()
        .unwrap_or_else(|error| error.into_inner());
    let mut payload = vec![0; byte_count];
    if start < bytes.len() {
        let available = (bytes.len() - start).min(byte_count);
        payload[..available].copy_from_slice(&bytes[start..start + available]);
    }
    Ok(payload)
}

fn ensure_writable_buffer(memory: &mut Memory, program: &Program, name: &str) -> Result<(), Error> {
    eval_expr::buffer_mut(memory, program, name).map(|_| ())
}

fn apply_async_transfer(
    transfer: AsyncTransfer,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), Error> {
    match transfer {
        AsyncTransfer::Copy {
            destination,
            start,
            payload,
        } => {
            let buffer = eval_expr::buffer_mut(memory, program, &destination)?;
            let mut bytes = buffer
                .bytes
                .write()
                .unwrap_or_else(|error| error.into_inner());
            if start >= bytes.len() {
                return Ok(());
            }
            let write_len = payload.len().min(bytes.len() - start);
            bytes[start..start + write_len].copy_from_slice(&payload[..write_len]);
            Ok(())
        }
    }
}

fn resolve_buffer<'a>(
    memory: &'a Memory,
    program: &Program,
    name: &str,
) -> Result<&'a oob::Buffer, Error> {
    let decl = program.buffer(name).ok_or_else(|| {
        Error::interp(format!(
            "missing buffer declaration `{name}`. Fix: declare every async transfer buffer."
        ))
    })?;
    if decl.access() == vyre::ir::BufferAccess::Workgroup {
        memory.workgroup.get(name)
    } else {
        memory.storage.get(name)
    }
    .ok_or_else(|| {
        Error::interp(format!(
            "missing buffer `{name}`. Fix: initialize every declared async transfer buffer."
        ))
    })
}

fn eval_if<'a>(
    cond: &Expr,
    then: &'a [Node],
    otherwise: &'a [Node],
    node: &Node,
    invocation: &mut Invocation<'a>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let cond_value = eval_expr::eval(cond, invocation, memory, program)?.truthy();
    if contains_barrier(then) || contains_barrier(otherwise) {
        invocation.uniform_checks.push((node_id(node), cond_value));
    }
    let branch = if cond_value { then } else { otherwise };
    invocation.push_scope();
    invocation.frames_mut().push(Frame::Nodes {
        nodes: branch,
        index: 0,
        scoped: true,
    });
    Ok(())
}

fn eval_loop<'a>(
    var: &'a str,
    from: &Expr,
    to: &Expr,
    body: &'a [Node],
    invocation: &mut Invocation<'a>,
    memory: &mut Memory,
    program: &Program,
) -> Result<(), vyre::Error> {
    let from_value = eval_expr::eval(from, invocation, memory, program)?;
    let to_value = eval_expr::eval(to, invocation, memory, program)?;
    let from = from_value.try_as_u32().ok_or_else(|| {
        Error::interp(format!(
                "loop lower bound {from_value:?} cannot be represented as u32. Fix: use an in-range unsigned loop bound."
        ))
    })?;
    let to = to_value.try_as_u32().ok_or_else(|| Error::interp(format!(
            "loop upper bound {to_value:?} cannot be represented as u32. Fix: use an in-range unsigned loop bound."
    )))?;
    invocation.frames_mut().push(Frame::Loop {
        var,
        next: from,
        to,
        body,
    });
    Ok(())
}

fn eval_return(invocation: &mut Invocation<'_>) -> Result<(), vyre::Error> {
    invocation.frames_mut().clear();
    invocation.returned = true;
    Ok(())
}

fn eval_block<'a>(nodes: &'a [Node], invocation: &mut Invocation<'a>) -> Result<(), vyre::Error> {
    invocation.push_scope();
    invocation.frames_mut().push(Frame::Nodes {
        nodes,
        index: 0,
        scoped: true,
    });
    Ok(())
}

fn eval_barrier(invocation: &mut Invocation<'_>) -> Result<(), vyre::Error> {
    invocation.waiting_at_barrier = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oob::Buffer;
    use crate::workgroup::InvocationIds;
    use vyre::ir::{BufferDecl, DataType};

    fn run_program(program: &Program, memory: &mut Memory) -> Result<(), vyre::Error> {
        let mut invocation = Invocation::new(InvocationIds::ZERO, program.entry());
        while !invocation.done() {
            step(&mut invocation, memory, program)?;
        }
        Ok(())
    }

    fn bytes(memory: &Memory, name: &str) -> Vec<u8> {
        memory
            .storage
            .get(name)
            .expect("Fix: test buffer exists")
            .bytes
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    #[test]
    fn async_load_wait_copies_payload_into_destination() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("src", 0, DataType::Bytes).with_count(8),
                BufferDecl::output("dst", 1, DataType::Bytes).with_count(8),
            ],
            [1, 1, 1],
            vec![
                Node::async_load_ext("src", "dst", Expr::u32(2), Expr::u32(4), "copy"),
                Node::AsyncWait { tag: "copy".into() },
            ],
        );
        let mut memory = Memory::empty()
            .with_storage(
                "src",
                Buffer::new(vec![10, 11, 12, 13, 14, 15, 16, 17], DataType::Bytes),
            )
            .with_storage("dst", Buffer::new(vec![0; 8], DataType::Bytes));

        run_program(&program, &mut memory).unwrap();

        assert_eq!(bytes(&memory, "dst"), vec![12, 13, 14, 15, 0, 0, 0, 0]);
    }

    #[test]
    fn async_store_wait_copies_payload_at_destination_offset() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("src", 0, DataType::Bytes).with_count(4),
                BufferDecl::output("dst", 1, DataType::Bytes).with_count(8),
            ],
            [1, 1, 1],
            vec![
                Node::async_store("src", "dst", Expr::u32(3), Expr::u32(4), "store"),
                Node::AsyncWait {
                    tag: "store".into(),
                },
            ],
        );
        let mut memory = Memory::empty()
            .with_storage("src", Buffer::new(vec![21, 22, 23, 24], DataType::Bytes))
            .with_storage("dst", Buffer::new(vec![0; 8], DataType::Bytes));

        run_program(&program, &mut memory).unwrap();

        assert_eq!(bytes(&memory, "dst"), vec![0, 0, 0, 21, 22, 23, 24, 0]);
    }
}
