//! Evaluation of `Expr::Call` for the reference interpreter.

use vyre::cpu_op::CpuFn;
use vyre::ir::{DataType, Expr, Program};
use vyre::Error;
use vyre::{dialect_lookup, TypedParam};

use crate::execution::expr_cast::spec_output_value;
use crate::{
    value::Value,
    workgroup::{Invocation, Memory, ResolvedCall},
};

const MAX_CALL_INPUT_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn eval_call(
    call_expr: *const Expr,
    op_id: &str,
    args: &[Expr],
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<Value, vyre::Error> {
    let resolved = resolve_call(call_expr, op_id, invocation)?;
    let def = resolved.def;

    {
        validate_arity(op_id, args.len(), def.signature.inputs.len())?;
        let input = encode_inputs(
            op_id,
            args,
            def.signature.inputs,
            invocation,
            memory,
            program,
        )?;

        let out_bytes = def
            .signature
            .outputs
            .first()
            .map(|p| match p.ty {
                "u32" | "i32" | "f32" | "vec-count" => 4,
                "u64" | "i64" | "f64" => 8,
                "u8" | "i8" | "bool" => 1,
                _ => 256,
            })
            .unwrap_or(256);
        let mut output = Vec::with_capacity(out_bytes);
        invoke_cpu_ref(op_id, def.lowerings.cpu_ref, &input, &mut output)?;

        let parsed_out_type = def
            .signature
            .outputs
            .first()
            .map(|p| match p.ty {
                "u32" => DataType::U32,
                "i32" => DataType::I32,
                "f32" => DataType::F32,
                "u8" => DataType::Bytes,
                "bool" => DataType::Bytes,
                _ => DataType::Bytes,
            })
            .unwrap_or(DataType::Bytes);

        Ok(spec_output_value(parsed_out_type, &output))
    }
}

pub(crate) fn invoke_cpu_ref(
    op_id: &str,
    cpu_ref: CpuFn,
    input: &[u8],
    output: &mut Vec<u8>,
) -> Result<(), Error> {
    let original_len = output.len();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cpu_ref(input, output))).map_err(
        |payload| {
            output.truncate(original_len);
            Error::interp(format!(
                "CPU reference for `{op_id}` panicked: {}. Fix: make the primitive reference total over byte inputs and return a structured error before registering it.",
                panic_payload_message(payload.as_ref())
            ))
        },
    )
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn resolve_call(
    call_expr: *const Expr,
    op_id: &str,
    invocation: &mut Invocation<'_>,
) -> Result<ResolvedCall, vyre::Error> {
    if let Some(resolved) = invocation.op_cache.get(&call_expr).copied() {
        return Ok(resolved);
    }
    let lookup = dialect_lookup().ok_or_else(|| {
        Error::interp(format!(
            "unsupported call `{op_id}`: no DialectLookup is installed. Fix: initialize vyre-driver before running the reference interpreter or inline the callee as IR."
        ))
    })?;
    let interned = lookup.intern_op(op_id);
    let def = lookup.lookup(interned).ok_or_else(|| {
        Error::interp(format!(
            "unsupported call `{op_id}`. Fix: register the op in DialectRegistry or inline the callee as IR."
        ))
    })?;
    let resolved = ResolvedCall { def };
    invocation.op_cache.insert(call_expr, resolved);
    Ok(resolved)
}

fn validate_arity(op_id: &str, actual: usize, expected: usize) -> Result<(), vyre::Error> {
    if actual == expected {
        return Ok(());
    }
    Err(Error::interp(format!(
        "call `{op_id}` received {actual} arguments but the primitive signature requires {expected}. Fix: pass exactly {expected} arguments."
    )))
}

fn encode_inputs(
    op_id: &str,
    args: &[Expr],
    inputs: &[TypedParam],
    invocation: &mut Invocation<'_>,
    memory: &mut Memory,
    program: &Program,
) -> Result<Vec<u8>, vyre::Error> {
    let mut input = Vec::with_capacity(inputs.iter().map(|param| param_width(param.ty)).sum());
    for (arg, param) in args.iter().zip(inputs) {
        let declared_width = param_width(param.ty);
        let next_len = input.len().checked_add(declared_width).ok_or_else(|| {
            Error::interp(format!(
                "call `{op_id}` input byte size overflows usize. Fix: reduce the argument count or byte payload size."
            ))
        })?;
        if next_len > MAX_CALL_INPUT_BYTES {
            return Err(Error::interp(format!(
                "call `{op_id}` requires {next_len} input bytes, exceeding the {MAX_CALL_INPUT_BYTES}-byte reference budget. Fix: reduce call input size."
            )));
        }
        let value = crate::execution::expr::eval(arg, invocation, memory, program)?;
        value.extend_bytes_width(declared_width, &mut input)?;
    }
    Ok(input)
}

fn param_width(ty: &str) -> usize {
    match ty {
        "u32" | "i32" | "f32" | "vec-count" => 4,
        "u64" | "i64" | "f64" => 8,
        "u8" | "i8" | "bool" => 1,
        _ => 1,
    }
}
