//! Expression evaluator that gives the parity engine a pure-Rust ground truth
//! for every `Expr` variant.
//!
//! If a backend lowers `Expr::BinOp`, `Expr::Load`, or `Expr::Atomic` differently
//! than this evaluator, the conform gate reports the exact divergence. This module
//! exists so IR semantics are defined by Rust code, not by whatever a backend
//! happens to emit.

use vyre::ir::{AtomicOp, BinOp, BufferAccess, BufferDecl, DataType, Expr, Program, UnOp};

use smallvec::SmallVec;
use vyre::Error;

use crate::execution::expr_cast::cast_value;
use crate::{atomics, oob, value::Value, workgroup::Invocation, workgroup::Memory};

/// Re-export the OOB-guarded buffer type used by storage operations.
pub use crate::oob::Buffer;

/// Evaluate an expression through the single-pass frame evaluator.
///
/// # Errors
///
/// Returns [`Error::Interp`] when expression lowering or flat execution
/// fails. The recursive evaluator is retained only as a test oracle.
pub fn eval(
    expr: &Expr,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<Value, vyre::Error> {
    eval_frame_oracle(expr, invocation, memory, program)
}

/// Evaluate an expression for one invocation.
///
/// # Errors
///
/// Returns [`Error::Interp`] on operand type errors, malformed atomic or call
/// expressions, unsupported variants, or float operands.
pub(crate) fn eval_frame_oracle(
    expr: &Expr,
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<Value, vyre::Error> {
    enum Frame<'a> {
        Expr(&'a Expr),
        BinOp(BinOp),
        UnOp(&'a UnOp),
        Select,
        Cast(&'a DataType),
        Fma,
        Load {
            buffer: &'a str,
        },
        AtomicIndex {
            op: AtomicOp,
            buffer: &'a str,
            expected: Option<&'a Expr>,
            value: &'a Expr,
        },
        AtomicExpected {
            op: AtomicOp,
            buffer: &'a str,
            index: u32,
            value: &'a Expr,
            expected_expr: &'a Expr,
        },
        AtomicValue {
            op: AtomicOp,
            buffer: &'a str,
            expected: Option<u32>,
            index: u32,
        },
    }

    let mut frames: SmallVec<[Frame<'_>; 32]> = SmallVec::new();
    frames.push(Frame::Expr(expr));
    let mut values: SmallVec<[Value; 32]> = SmallVec::new();

    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(expr) => match expr {
                Expr::LitU32(value) => values.push(Value::U32(*value)),
                Expr::LitI32(value) => values.push(Value::I32(*value)),
                Expr::LitF32(value) => {
                    values.push(Value::Float(f64::from(
                        crate::execution::typed_ops::canonical_f32(*value),
                    )));
                }
                Expr::LitBool(value) => values.push(Value::Bool(*value)),
                Expr::Var(name) => values.push(eval_var(name, invocation)?),
                Expr::BufLen { buffer } => values.push(eval_buf_len(buffer, memory, program)?),
                Expr::InvocationId { axis } => values.push(eval_invocation_id(*axis, invocation)?),
                Expr::WorkgroupId { axis } => values.push(eval_workgroup_id(*axis, invocation)?),
                Expr::LocalId { axis } => values.push(eval_local_id(*axis, invocation)?),
                Expr::Load { buffer, index } => {
                    frames.push(Frame::Load { buffer });
                    frames.push(Frame::Expr(index));
                }
                Expr::BinOp { op, left, right } => {
                    frames.push(Frame::BinOp(*op));
                    frames.push(Frame::Expr(right));
                    frames.push(Frame::Expr(left));
                }
                Expr::UnOp { op, operand } => {
                    frames.push(Frame::UnOp(op));
                    frames.push(Frame::Expr(operand));
                }
                Expr::Select {
                    cond,
                    true_val,
                    false_val,
                } => {
                    frames.push(Frame::Select);
                    frames.push(Frame::Expr(false_val));
                    frames.push(Frame::Expr(true_val));
                    frames.push(Frame::Expr(cond));
                }
                Expr::Cast { target, value } => {
                    frames.push(Frame::Cast(target));
                    frames.push(Frame::Expr(value));
                }
                Expr::Fma { a, b, c } => {
                    frames.push(Frame::Fma);
                    frames.push(Frame::Expr(c));
                    frames.push(Frame::Expr(b));
                    frames.push(Frame::Expr(a));
                }
                Expr::Atomic {
                    op,
                    buffer,
                    index,
                    expected,
                    value,
                    ordering: _,
                } => {
                    match (*op, expected.as_deref()) {
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
                    frames.push(Frame::AtomicIndex {
                        op: *op,
                        buffer,
                        expected: expected.as_deref(),
                        value,
                    });
                    frames.push(Frame::Expr(index));
                }
                Expr::Call { op_id, args } => {
                    let val = crate::execution::call::eval_call(
                        expr as *const Expr,
                        op_id,
                        args,
                        invocation,
                        memory,
                        program,
                    )?;
                    values.push(val);
                }
                Expr::Opaque(extension) => {
                    return Err(Error::interp(format!(
                        "reference interpreter does not support opaque expression extension `{}`/`{}`. Fix: provide a reference evaluator for this ExprNode or lower it to core Expr variants before evaluation.",
                        extension.extension_kind(),
                        extension.debug_identity()
                    )));
                }
                _ => {
                    return Err(Error::interp(
                        "reference interpreter encountered an unknown expression variant. Fix: add explicit reference semantics for the new ExprNode before dispatch.",
                    ));
                }
            },
            Frame::BinOp(op) => {
                let right = values.pop().ok_or_else(|| {
                    Error::interp("binary op missing right operand. Fix: internal evaluator error.")
                })?;
                let left = values.pop().ok_or_else(|| {
                    Error::interp("binary op missing left operand. Fix: internal evaluator error.")
                })?;
                values.push(super::typed_ops::eval_binop(op, left, right)?);
            }
            Frame::UnOp(op) => {
                let operand = values.pop().ok_or_else(|| {
                    Error::interp("unary op missing operand. Fix: internal evaluator error.")
                })?;
                values.push(super::typed_ops::eval_unop(op, operand)?);
            }
            Frame::Select => {
                let false_val = values.pop().ok_or_else(|| {
                    Error::interp("select missing false branch. Fix: internal evaluator error.")
                })?;
                let true_val = values.pop().ok_or_else(|| {
                    Error::interp("select missing true branch. Fix: internal evaluator error.")
                })?;
                let cond = values
                    .pop()
                    .ok_or_else(|| {
                        Error::interp("select missing condition. Fix: internal evaluator error.")
                    })?
                    .truthy();
                values.push(if cond { true_val } else { false_val });
            }
            Frame::Cast(target) => {
                let value = values.pop().ok_or_else(|| {
                    Error::interp("cast missing value. Fix: internal evaluator error.")
                })?;
                values.push(cast_value(target, &value)?);
            }
            Frame::Fma => {
                let c = values
                    .pop()
                    .ok_or_else(|| {
                        Error::interp("fma missing operand c. Fix: internal evaluator error.")
                    })?
                    .try_as_f32()
                    .ok_or_else(|| {
                        Error::interp(
                            "fma operand `c` is not a float. Fix: cast to f32 before fma.",
                        )
                    })?;
                let b = values
                    .pop()
                    .ok_or_else(|| {
                        Error::interp("fma missing operand b. Fix: internal evaluator error.")
                    })?
                    .try_as_f32()
                    .ok_or_else(|| {
                        Error::interp(
                            "fma operand `b` is not a float. Fix: cast to f32 before fma.",
                        )
                    })?;
                let a = values
                    .pop()
                    .ok_or_else(|| {
                        Error::interp("fma missing operand a. Fix: internal evaluator error.")
                    })?
                    .try_as_f32()
                    .ok_or_else(|| {
                        Error::interp(
                            "fma operand `a` is not a float. Fix: cast to f32 before fma.",
                        )
                    })?;
                let a = crate::execution::typed_ops::canonical_f32(a);
                let b = crate::execution::typed_ops::canonical_f32(b);
                let c = crate::execution::typed_ops::canonical_f32(c);
                values.push(Value::Float(f64::from(
                    crate::execution::typed_ops::canonical_f32(a.mul_add(b, c)),
                )));
            }
            Frame::Load { buffer } => {
                let value = values.pop().ok_or_else(|| {
                    Error::interp("load missing index. Fix: internal evaluator error.")
                })?;
                let idx = value.try_as_u32().ok_or_else(|| {
                    Error::interp(format!(
                        "load index {value:?} cannot be represented as u32. Fix: use a non-negative scalar index within u32."
                    ))
                })?;
                values.push(oob::load(resolve_buffer(memory, program, buffer)?, idx));
            }
            Frame::AtomicIndex {
                op,
                buffer,
                expected,
                value,
            } => {
                let val = values.pop().ok_or_else(|| {
                    Error::interp("atomic missing index. Fix: internal evaluator error.")
                })?;
                let idx = val.try_as_u32().ok_or_else(|| {
                    Error::interp(format!(
                        "atomic index {val:?} cannot be represented as u32. Fix: use a non-negative scalar index within u32."
                    ))
                })?;
                if let Some(expected_expr) = expected {
                    frames.push(Frame::AtomicExpected {
                        op,
                        buffer,
                        index: idx,
                        value,
                        expected_expr,
                    });
                    frames.push(Frame::Expr(expected_expr));
                } else {
                    frames.push(Frame::AtomicValue {
                        op,
                        buffer,
                        expected: None,
                        index: idx,
                    });
                    frames.push(Frame::Expr(value));
                }
            }
            Frame::AtomicExpected {
                op,
                buffer,
                index,
                value,
                expected_expr,
            } => {
                let val = values.pop().ok_or_else(|| {
                    Error::interp(
                        "atomic compare-exchange missing expected value. Fix: internal evaluator error.",
                    )
                })?;
                let expected_val = val.try_as_u32().ok_or_else(|| {
                    Error::interp(format!(
                        "atomic expected value {expected_expr:?} cannot be represented as u32. Fix: use a scalar u32-compatible argument."
                    ))
                })?;
                frames.push(Frame::AtomicValue {
                    op,
                    buffer,
                    expected: Some(expected_val),
                    index,
                });
                frames.push(Frame::Expr(value));
            }
            Frame::AtomicValue {
                op,
                buffer,
                expected,
                index,
            } => {
                let val = values.pop().ok_or_else(|| {
                    Error::interp("atomic missing value. Fix: internal evaluator error.")
                })?;
                let value = val.try_as_u32().ok_or_else(|| {
                    Error::interp(
                        "atomic value cannot be represented as u32. Fix: use a scalar u32-compatible argument.",
                    )
                })?;
                let target = atomic_buffer_mut(memory, program, buffer)?;
                let Some(old) = oob::atomic_load(target, index) else {
                    values.push(Value::U32(0));
                    continue;
                };
                let (old, new) = atomics::apply(op, old, expected, value)?;
                oob::atomic_store(target, index, new);
                values.push(Value::U32(old));
            }
        }
    }

    values.pop().ok_or_else(|| {
        Error::interp("expression evaluation produced no value. Fix: internal evaluator error.")
    })
}

/// Return a mutable buffer only when the program declares it writable.
///
/// # Errors
///
/// Returns [`Error::Interp`] if the buffer is read-only, uniform,
/// or does not exist in the program declaration.
pub fn buffer_mut<'a>(
    memory: &'a mut Memory,
    program: &Program,
    name: &str,
) -> Result<&'a mut Buffer, vyre::Error> {
    let decl = buffer_decl(program, name)?;
    match decl.access() {
        BufferAccess::ReadWrite | BufferAccess::WriteOnly | BufferAccess::Workgroup => {
            resolve_buffer_mut(memory, decl)
        }
        BufferAccess::ReadOnly | BufferAccess::Uniform => Err(Error::interp(format!(
            "store target `{name}` is not writable. Fix: declare it ReadWrite, WriteOnly, or Workgroup."
        ))),
        _ => Err(Error::interp(format!(
            "store target `{name}` uses an unsupported access mode. Fix: use a supported BufferAccess."
        ))),
    }
}

fn eval_var(name: &str, invocation: &Invocation<'_>) -> Result<Value, vyre::Error> {
    invocation.local(name).cloned().ok_or_else(|| {
        Error::interp(format!(
            "reference to undeclared variable `{name}`. Fix: add a Let before this use."
        ))
    })
}

fn eval_buf_len(buffer: &str, memory: &Memory, program: &Program) -> Result<Value, vyre::Error> {
    Ok(Value::U32(resolve_buffer(memory, program, buffer)?.len()))
}

fn eval_invocation_id(axis: u8, invocation: &Invocation<'_>) -> Result<Value, vyre::Error> {
    axis_value(invocation.ids.global, axis)
}

fn eval_workgroup_id(axis: u8, invocation: &Invocation<'_>) -> Result<Value, vyre::Error> {
    axis_value(invocation.ids.workgroup, axis)
}

fn eval_local_id(axis: u8, invocation: &Invocation<'_>) -> Result<Value, vyre::Error> {
    axis_value(invocation.ids.local, axis)
}

fn resolve_buffer<'a>(
    memory: &'a Memory,
    program: &Program,
    name: &str,
) -> Result<&'a oob::Buffer, vyre::Error> {
    let decl = buffer_decl(program, name)?;
    if decl.access() == BufferAccess::Workgroup {
        memory.workgroup.get(name)
    } else {
        memory.storage.get(name)
    }
    .ok_or_else(|| {
        Error::interp(format!(
            "missing buffer `{name}`. Fix: initialize all declared buffers."
        ))
    })
}

fn resolve_buffer_mut<'a>(
    memory: &'a mut Memory,
    decl: &BufferDecl,
) -> Result<&'a mut oob::Buffer, vyre::Error> {
    let name = decl.name();
    if decl.access() == BufferAccess::Workgroup {
        memory.workgroup.get_mut(name)
    } else {
        memory.storage.get_mut(name)
    }
    .ok_or_else(|| {
        Error::interp(format!(
            "missing buffer `{name}`. Fix: initialize all declared buffers."
        ))
    })
}

fn atomic_buffer_mut<'a>(
    memory: &'a mut Memory,
    program: &Program,
    name: &str,
) -> Result<&'a mut oob::Buffer, vyre::Error> {
    let decl = buffer_decl(program, name)?;
    match decl.access() {
        BufferAccess::ReadWrite => resolve_buffer_mut(memory, decl),
        BufferAccess::Workgroup => Err(Error::interp(format!(
            "atomic target `{name}` is workgroup memory. Fix: atomics only support ReadWrite storage buffers."
        ))),
        BufferAccess::ReadOnly | BufferAccess::Uniform => Err(Error::interp(format!(
            "atomic target `{name}` is not writable. Fix: atomics only support ReadWrite storage buffers."
        ))),
        _ => Err(Error::interp(format!(
            "atomic target `{name}` uses an unsupported access mode. Fix: use a supported BufferAccess."
        ))),
    }
}

fn buffer_decl<'a>(program: &'a Program, name: &str) -> Result<&'a BufferDecl, vyre::Error> {
    program.buffer(name).ok_or_else(|| {
        Error::interp(format!(
            "unknown buffer `{name}`. Fix: declare it in Program::buffers."
        ))
    })
}

fn axis_value(values: [u32; 3], axis: u8) -> Result<Value, vyre::Error> {
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

#[cfg(test)]
mod tests {

    use proptest::prelude::*;
    use vyre::ir::{Expr, Program};

    use super::eval;
    use crate::value::Value;
    use crate::workgroup::{Invocation, InvocationIds, Memory};

    fn empty_memory() -> Memory {
        Memory {
            storage: Default::default(),
            workgroup: Default::default(),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn prop_frame_evaluator_matches_recursive_contract(a in any::<u32>(), b in any::<u32>(), c in any::<u32>(), pick_left in any::<bool>()) {
            let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
            let int_expr = Expr::select(
                Expr::bool(pick_left),
                Expr::add(Expr::u32(a), Expr::mul(Expr::u32(b), Expr::u32(c))),
                Expr::sub(Expr::u32(a), Expr::u32(b)),
            );
            let float_expr = Expr::fma(
                Expr::f32(((a & 0xffff) as f32) * 0.5),
                Expr::f32(((b & 0xff) as f32) + 1.0),
                Expr::f32(((c & 0xffff) as f32) * 0.25),
            );

            for expr in [&int_expr, &float_expr] {
                let mut invocation = Invocation::new(InvocationIds::ZERO, program.entry());
                let mut memory = empty_memory();

                let frame = eval(expr, &mut invocation, &mut memory, &program)
                    .expect("Fix: frame evaluator must evaluate generated expression");
                let recursive = eval_recursive_contract(expr)
                    .expect("Fix: recursive contract must evaluate generated expression");
                prop_assert_eq!(frame, recursive);
            }
        }
    }

    #[test]
    fn deeply_nested_expression_uses_frame_stack_not_host_recursion() {
        let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
        let mut expr = Expr::u32(0);
        for _ in 0..4096 {
            expr = Expr::add(expr, Expr::u32(1));
        }

        let mut invocation = Invocation::new(InvocationIds::ZERO, program.entry());
        let mut memory = empty_memory();
        let value = eval(&expr, &mut invocation, &mut memory, &program).expect(
            "Fix: frame evaluator must handle deep generated expressions without recursion",
        );

        assert_eq!(value, Value::U32(4096));
    }

    fn eval_recursive_contract(expr: &Expr) -> Result<Value, vyre::Error> {
        match expr {
            Expr::LitU32(value) => Ok(Value::U32(*value)),
            Expr::LitI32(value) => Ok(Value::I32(*value)),
            Expr::LitF32(value) => Ok(Value::Float(f64::from(
                crate::execution::typed_ops::canonical_f32(*value),
            ))),
            Expr::LitBool(value) => Ok(Value::Bool(*value)),
            Expr::BinOp { op, left, right } => {
                let left = eval_recursive_contract(left)?;
                let right = eval_recursive_contract(right)?;
                crate::execution::typed_ops::eval_binop(*op, left, right)
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                if eval_recursive_contract(cond)?.truthy() {
                    eval_recursive_contract(true_val)
                } else {
                    eval_recursive_contract(false_val)
                }
            }
            Expr::Fma { a, b, c } => {
                let a = eval_recursive_contract(a)?.try_as_f32().ok_or_else(|| {
                    vyre::Error::interp("fma operand `a` is not a float in recursive contract")
                })?;
                let b = eval_recursive_contract(b)?.try_as_f32().ok_or_else(|| {
                    vyre::Error::interp("fma operand `b` is not a float in recursive contract")
                })?;
                let c = eval_recursive_contract(c)?.try_as_f32().ok_or_else(|| {
                    vyre::Error::interp("fma operand `c` is not a float in recursive contract")
                })?;
                let a = crate::execution::typed_ops::canonical_f32(a);
                let b = crate::execution::typed_ops::canonical_f32(b);
                let c = crate::execution::typed_ops::canonical_f32(c);
                Ok(Value::Float(f64::from(
                    crate::execution::typed_ops::canonical_f32(a.mul_add(b, c)),
                )))
            }
            _ => Err(vyre::Error::interp(
                "recursive test contract received an expression outside its generated subset",
            )),
        }
    }
}
