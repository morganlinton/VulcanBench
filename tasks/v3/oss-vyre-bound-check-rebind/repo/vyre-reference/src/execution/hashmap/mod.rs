//! HashMap-backed reference interpreter split into execution, state, memory,
//! synchronization, and optional subgroup semantics.
//!
//! This root module owns expression evaluation and the split modules own their
//! state, memory, execution, synchronization, and subgroup contracts.

pub(crate) mod memory;
pub(crate) mod state;
pub(crate) mod step;
pub(crate) mod subgroup;
pub(crate) mod sync;

use memory::{atomic_buffer_mut, output_value, resolve_buffer, HashmapMemory};
#[cfg(feature = "subgroup-ops")]
use state::HashmapInvocationSnapshot;
use state::{create_invocations, run_invocations, HashmapInvocation};
use step::{axis_value, eval_call, eval_to_index};
#[cfg(feature = "subgroup-ops")]
use subgroup::{eval_subgroup_ballot, eval_subgroup_reduce, eval_subgroup_shuffle};
use sync::element_count;

use crate::{
    atomics,
    oob::{self, Buffer},
    value::Value,
};
use rustc_hash::FxHashMap;
use vyre::ir::{AtomicOp, BufferAccess, Expr, Program};
use vyre::Error;

#[doc = " Execute a vyre IR program using hashmap-backed locals."]
pub(crate) fn run_hashmap_reference(
    program: &Program,
    inputs: &[Value],
) -> Result<Vec<Value>, Error> {
    #[cfg(feature = "subgroup-ops")]
    let validation_report = vyre::validate::validate::validate_with_options(
        program,
        vyre::validate::ValidationOptions::default().with_backend_capabilities(
            vyre::validate::BackendCapabilities {
                supports_subgroup_ops: true,
                ..Default::default()
            },
        ),
    );
    #[cfg(not(feature = "subgroup-ops"))]
    let validation_report = vyre::validate::validate::validate_with_options(
        program,
        vyre::validate::ValidationOptions::default(),
    );
    let validation_errors = validation_report.errors;
    if !validation_errors.is_empty() {
        let message_len = validation_errors
            .iter()
            .map(|error| error.message().len())
            .sum::<usize>()
            + validation_errors.len().saturating_sub(1) * 2;
        let mut messages = String::with_capacity(message_len);
        for (index, error) in validation_errors.iter().enumerate() {
            if index != 0 {
                messages.push_str("; ");
            }
            messages.push_str(error.message());
        }
        return Err(Error::interp(format!(
            "program failed IR validation: {messages}. Fix: repair the Program before invoking the reference interpreter."
        )));
    }
    let mut storage = FxHashMap::default();
    let backend_allocated_output = |decl: &vyre::ir::BufferDecl| {
        decl.is_output()
            || decl.access() == BufferAccess::WriteOnly
            || (decl.is_pipeline_live_out() && decl.access() == BufferAccess::ReadWrite)
    };
    let logical_input_count = program
        .buffers()
        .iter()
        .filter(|decl| decl.access() != BufferAccess::Workgroup && !backend_allocated_output(decl))
        .count();
    let legacy_input_count = program
        .buffers()
        .iter()
        .filter(|decl| decl.access() != BufferAccess::Workgroup)
        .count();
    let legacy_input_mode =
        inputs.len() == legacy_input_count && inputs.len() != logical_input_count;
    let mut input_index = 0usize;
    let mut output_decls = Vec::new();
    let mut max_output_elements = 0u32;
    let mut max_input_elements = 1u32;
    let mut program_graph_node_count = None;
    let mut has_workgroup_buffer = false;
    for decl in program.buffers() {
        if decl.access() == BufferAccess::Workgroup {
            has_workgroup_buffer = true;
            continue;
        }
        if decl.binding() == 0 && decl.name() == "pg_nodes" {
            program_graph_node_count = Some(decl.count());
        }
        let required_bytes = declared_min_byte_len(decl)?;
        let is_backend_allocated_output = backend_allocated_output(decl);
        let bytes = if is_backend_allocated_output {
            if legacy_input_mode {
                let _legacy_output_initializer = inputs.get(input_index).ok_or_else(|| {
                    Error::interp(format!(
                        "missing legacy output initializer for buffer `{}`. Fix: pass one Value for each non-workgroup buffer or migrate to logical inputs only.",
                        decl.name()
                    ))
                })?;
                input_index += 1;
            }
            vec![0u8; required_bytes]
        } else {
            let value = inputs.get(input_index).ok_or_else(|| {
                Error::interp(format!(
                    "missing input for buffer `{}`. Fix: pass one Value for each non-output, non-workgroup buffer in Program::buffers order.",
                    decl.name()
                ))
            })?;
            input_index += 1;
            value.to_bytes()
        };
        if bytes.len() < required_bytes {
            return Err(Error::interp(format!(
                "buffer `{}` has {} bytes but requires at least {} bytes ({} elements of {}). Fix: provide a larger input buffer.",
                decl.name(),
                bytes.len(),
                required_bytes,
                decl.count(),
                decl.element()
            )));
        }
        let elements = element_count(decl, bytes.len())?;
        if is_backend_allocated_output || decl.access() == BufferAccess::ReadWrite {
            max_output_elements = max_output_elements.max(elements);
            output_decls.push(decl.clone());
        } else {
            max_input_elements = max_input_elements.max(elements);
        }
        storage.insert(
            decl.name().to_string(),
            Buffer::new(bytes, decl.element().clone()),
        );
    }
    if input_index != inputs.len() {
        return Err(Error::interp(
            "unused input values supplied. Fix: pass exactly one Value per non-workgroup buffer declaration.",
        ));
    }
    if program.workgroup_size().contains(&0) {
        return Err(Error::interp(
            "workgroup size contains zero. Fix: all dimensions must be >= 1.",
        ));
    }
    let [sx, sy, sz] = program.workgroup_size();
    let invocations_per_workgroup = [sx, sy, sz]
        .iter()
        .copied()
        .fold(1u32, u32::saturating_mul)
        .max(1);
    let force_full_span = has_workgroup_buffer || program.stats().atomic_op_count > 0;
    let dispatch_elements = max_output_elements
        .max(program_graph_node_count.unwrap_or(0))
        .max(1)
        .max(if output_decls.is_empty() || force_full_span {
            max_input_elements
        } else {
            1
        });
    let total_wg = dispatch_elements.div_ceil(invocations_per_workgroup).max(1);
    let active: Vec<usize> = [sx, sy, sz]
        .iter()
        .enumerate()
        .filter(|(_, size)| **size > 1)
        .map(|(i, _)| i)
        .collect();
    let n = active.len().max(1);
    let mut counts = [1u32, 1, 1];
    if active.is_empty() {
        counts[0] = total_wg;
    } else {
        let base = (total_wg as f64).powf(1.0 / n as f64).ceil() as u32;
        for &axis in &active {
            counts[axis] = base.max(1);
        }
    }
    let [workgroup_count_x, workgroup_count_y, workgroup_count_z] = counts;
    let entry = program.entry();
    #[cfg(feature = "subgroup-ops")]
    let uses_subgroup_ops = vyre::program_caps::scan(program).subgroup_ops;
    let mut memory = HashmapMemory::new(storage);
    for wg_z in 0..workgroup_count_z {
        for wg_y in 0..workgroup_count_y {
            for wg_x in 0..workgroup_count_x {
                memory.reset_workgroup(program)?;
                let mut invocations = create_invocations(program, [wg_x, wg_y, wg_z], entry)?;
                run_invocations(
                    &mut memory,
                    &mut invocations,
                    #[cfg(feature = "subgroup-ops")]
                    uses_subgroup_ops,
                )?;
            }
        }
    }
    let mut storage = memory.storage;
    output_decls . into_iter () . map (| decl | { storage . remove (decl . name ()) . map (| buffer | output_value (buffer , & decl)) . ok_or_else (| | { let name = decl . name () ; Error :: interp (format ! ("missing output buffer `{name}` after dispatch. Fix: keep buffer declarations unique.")) }) }) . collect ()
}

fn declared_min_byte_len(decl: &vyre::ir::BufferDecl) -> Result<usize, Error> {
    match decl.static_byte_len() {
        Ok(Some(byte_len)) => Ok(byte_len),
        Ok(None) if decl.count() == 0 => Ok(0),
        Ok(None) => Err(Error::interp(format!(
            "buffer `{}` has unsized element type {}. Fix: provide a fixed-width buffer element type before invoking the reference interpreter.",
            decl.name(),
            decl.element()
        ))),
        Err(error) => Err(Error::interp(error)),
    }
}

fn eval_expr(
    expr: &Expr,
    invocation: &mut HashmapInvocation<'_>,
    memory: &mut HashmapMemory,
    #[cfg(feature = "subgroup-ops")] snapshots: &[HashmapInvocationSnapshot],
) -> Result<Value, Error> {
    match expr {
        Expr::LitU32(value) => Ok(Value::U32(*value)),
        Expr::LitI32(value) => Ok(Value::I32(*value)),
        Expr::LitF32(value) => Ok(Value::Float(f64::from(crate::execution::typed_ops::canonical_f32(
            *value,
        )))),
        Expr::LitBool(value) => Ok(Value::Bool(*value)),
        Expr::Var(name) => invocation.locals.local(name).ok_or_else(|| {
            Error::interp(format!(
                "reference to undeclared variable `{name}`. Fix: add a Let before this use."
            ))
        }),
        Expr::Load { buffer, index } => {
            let idx = eval_to_index(
                index,
                "load index",
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            Ok(oob::load(resolve_buffer(memory, buffer)?, idx))
        }
        Expr::BufLen { buffer } => Ok(Value::U32(resolve_buffer(memory, buffer)?.len())),
        Expr::InvocationId { axis } => axis_value(invocation.ids.global, *axis),
        Expr::WorkgroupId { axis } => axis_value(invocation.ids.workgroup, *axis),
        Expr::LocalId { axis } => axis_value(invocation.ids.local, *axis),
        Expr::BinOp { op, left, right } => {
            let left = eval_expr(
                left,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            let right = eval_expr(
                right,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            crate::execution::typed_ops::eval_binop(*op, left, right)
        }
        Expr::UnOp { op, operand } => {
            let operand = eval_expr(
                operand,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            crate::execution::typed_ops::eval_unop(op, operand)
        }
        Expr::Call { op_id, args } => eval_call(
            expr as *const Expr,
            op_id,
            args,
            invocation,
            memory,
            #[cfg(feature = "subgroup-ops")]
            snapshots,
        ),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            let cond = eval_expr(
                cond,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?
            .truthy();
            let true_val = eval_expr(
                true_val,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            let false_val = eval_expr(
                false_val,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            Ok(if cond { true_val } else { false_val })
        }
        Expr::Cast { target, value } => {
            let value = eval_expr(
                value,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?;
            crate::execution::expr_cast::cast_value(target, &value)
        }
        Expr::Fma { a, b, c } => {
            let a = eval_expr(
                a,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?
            .try_as_f32()
            .ok_or_else(|| {
                Error::interp("fma operand `a` is not a float. Fix: cast to f32 before fma.")
            })?;
            let b = eval_expr(
                b,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?
            .try_as_f32()
            .ok_or_else(|| {
                Error::interp("fma operand `b` is not a float. Fix: cast to f32 before fma.")
            })?;
            let c = eval_expr(
                c,
                invocation,
                memory,
                #[cfg(feature = "subgroup-ops")]
                snapshots,
            )?
            .try_as_f32()
            .ok_or_else(|| {
                Error::interp("fma operand `c` is not a float. Fix: cast to f32 before fma.")
            })?;
            let a = crate::execution::typed_ops::canonical_f32(a);
            let b = crate::execution::typed_ops::canonical_f32(b);
            let c = crate::execution::typed_ops::canonical_f32(c);
            Ok(Value::Float(f64::from(crate::execution::typed_ops::canonical_f32(
                a.mul_add(b, c),
            ))))
        }
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering: _,
        } => eval_atomic(
            *op,
            buffer,
            index,
            expected.as_deref(),
            value,
            invocation,
            memory,
            #[cfg(feature = "subgroup-ops")]
            snapshots,
        ),
        Expr::Opaque(extension) => Err(Error::interp(format!(
            "hashmap reference interpreter does not support opaque expression extension `{}`/`{}`. Fix: provide a reference evaluator for this ExprNode or lower it to core Expr variants before evaluation.",
            extension.extension_kind(),
            extension.debug_identity()
        ))),
        Expr::SubgroupBallot { cond } => {
            #[cfg(feature = "subgroup-ops")]
            {
                eval_subgroup_ballot(cond, invocation, snapshots, memory)
            }
            #[cfg(not(feature = "subgroup-ops"))]
            {
                let cond = eval_expr(cond, invocation, memory)?.truthy();
                Ok(Value::U32(u32::from(cond)))
            }
        }
        Expr::SubgroupShuffle { value, lane } => {
            #[cfg(feature = "subgroup-ops")]
            {
                eval_subgroup_shuffle(value, lane, invocation, snapshots, memory)
            }
            #[cfg(not(feature = "subgroup-ops"))]
            {
                let value_val = eval_expr(value, invocation, memory)?;
                let lane_val = eval_expr(lane, invocation, memory)?;
                let lane_u32 = lane_val . try_as_u32 () . ok_or_else (| | { Error :: interp ("subgroup_shuffle lane index is not a u32. Fix: use a scalar u32 lane argument." ,) }) ? ;
                Ok(if lane_u32 == 0 {
                    value_val
                } else {
                    Value::U32(0)
                })
            }
        }
        Expr::SubgroupReduce { op, value } => {
            #[cfg(feature = "subgroup-ops")]
            {
                eval_subgroup_reduce(*op, value, invocation, snapshots, memory)
            }
            #[cfg(not(feature = "subgroup-ops"))]
            {
                // Single-lane interpreter: a reduction over one lane is that
                // lane's value for every operator (Add/Mul/Min/Max/And/Or/Xor).
                let _ = op;
                eval_expr(value, invocation, memory)
            }
        }
        _ => Err(Error::interp(
            "hashmap reference interpreter encountered an unknown expression variant. Fix: add explicit reference semantics for the new ExprNode before dispatch.",
        )),
    }
}
#[allow(clippy::too_many_arguments)]
fn eval_atomic(
    op: AtomicOp,
    buffer: &str,
    index: &Expr,
    expected: Option<&Expr>,
    value: &Expr,
    invocation: &mut HashmapInvocation<'_>,
    memory: &mut HashmapMemory,
    #[cfg(feature = "subgroup-ops")] snapshots: &[HashmapInvocationSnapshot],
) -> Result<Value, Error> {
    match (op, expected) {
        (AtomicOp::CompareExchange, None) => {
            return Err(Error::interp(
                "compare-exchange atomic is missing expected value. Fix: set Expr::Atomic.expected for AtomicOp::CompareExchange.",
            ));
        }
        (AtomicOp::CompareExchange, Some(_)) => {}
        (_, Some(_)) => {
            return Err(Error::interp(
                "non-compare-exchange atomic includes an expected value. Fix: use Expr::Atomic.expected only with AtomicOp::CompareExchange.",
            ));
        }
        (_, None) => {}
    }
    let idx = eval_to_index(
        index,
        "atomic index",
        invocation,
        memory,
        #[cfg(feature = "subgroup-ops")]
        snapshots,
    )?;
    let expected = expected . map (| expr | { eval_expr (expr , invocation , memory , #[cfg (feature = "subgroup-ops")] snapshots ,) ? . try_as_u32 () . ok_or_else (| | { Error :: interp (format ! ("atomic expected value {expr:?} cannot be represented as u32. Fix: use a scalar u32-compatible argument.")) }) }) . transpose () ? ;
    let value = eval_expr(
        value,
        invocation,
        memory,
        #[cfg(feature = "subgroup-ops")]
        snapshots,
    )?;
    let value = value.try_as_u32().ok_or_else(|| {
        Error::interp(
            "atomic value cannot be represented as u32. Fix: use a scalar u32-compatible argument.",
        )
    })?;
    let target = atomic_buffer_mut(memory, buffer)?;
    let Some(old) = oob::atomic_load(target, idx) else {
        return Ok(Value::U32(0));
    };
    let (old, new) = atomics::apply(op, old, expected, value)?;
    oob::atomic_store(target, idx, new);
    Ok(Value::U32(old))
}
