//! RELEASE TEST LANE 18  -  per-op F32 ULP audit.
//!
//! For every registered op whose fixtures contain F32 buffers:
//!   1. Dispatch the fixture through a linked dispatch-capable backend.
//!   2. Compute max ULP delta against CPU reference per output element.
//!   3. Assert delta ≤ the explicit F32 ULP budget for the program.
//!   4. Adversarial companion: feed finite normal values, signed zero, infinities,
//!      NaN, max finite, and denormals into every F32 input buffer. Finite normal
//!      companions assert the ULP bound. Architecture-edge companions assert
//!      successful dispatch and output shape while still recording observed ULP.
//!   5. Print a table of max-ULP-observed per op so regressions are visible.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use inventory::iter;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre::VyreBackend;
use vyre_conform_runner::dispatch_grid;
use vyre_conform_runner::fp_parity::{f32_ulp_tolerance, ulp_distance};
use vyre_foundation::program_caps;
use vyre_reference::value::Value;

#[cfg(feature = "gpu")]
use vyre_driver_cuda as _;
#[cfg(feature = "gpu")]
use vyre_driver_metal as _;
#[cfg(feature = "gpu")]
use vyre_driver_wgpu as _;
use vyre_intrinsics as _;
use vyre_libs as _;
use vyre_primitives as _;

type FixtureCases = Vec<Vec<Vec<u8>>>;
type FixtureFn = fn() -> FixtureCases;

const TRANSCENDENTAL_F32_ULP_BUDGET: u32 = 128;

#[path = "__split/ulp_audit_input_plan.rs"]
mod ulp_audit_input_plan;

pub(crate) use ulp_audit_input_plan::*;

struct UnifiedEntry {
    id: &'static str,
    build: fn() -> Program,
    test_inputs: Option<FixtureFn>,
    expected_output: Option<FixtureFn>,
}

fn all_entries() -> Vec<UnifiedEntry> {
    let libs = iter::<vyre_libs::harness::OpEntry>
        .into_iter()
        .map(|entry| UnifiedEntry {
            id: entry.id,
            build: entry.build,
            test_inputs: entry.test_inputs,
            expected_output: entry.expected_output,
        });
    let intrinsics = iter::<vyre_intrinsics::harness::OpEntry>
        .into_iter()
        .map(|entry| UnifiedEntry {
            id: entry.id,
            build: entry.build,
            test_inputs: entry.test_inputs,
            expected_output: entry.expected_output,
        });
    let primitives = iter::<vyre_primitives::harness::OpEntry>
        .into_iter()
        .map(|entry| UnifiedEntry {
            id: entry.id,
            build: entry.build,
            test_inputs: entry.test_inputs,
            expected_output: entry.expected_output,
        });

    let mut entries: Vec<UnifiedEntry> = libs.chain(intrinsics).chain(primitives).collect();
    entries.sort_by(|a, b| a.id.cmp(b.id));
    entries
}

fn run_cpu_from_slices<'a>(
    program: &Program,
    inputs: &[&[u8]],
    values: &'a mut Vec<Value>,
    outputs: &'a mut Vec<Vec<u8>>,
) -> Result<&'a [Vec<u8>], String> {
    values.clear();
    for input in inputs {
        values.push(Value::from(*input));
    }
    let evaluated = vyre_reference::reference_eval(program, values).map_err(|e| e.to_string())?;
    outputs.clear();
    outputs.extend(evaluated.into_iter().map(|v| v.to_bytes()));
    Ok(outputs.as_slice())
}

fn backend_inputs_from_vectors<'a>(buffers: &'a [Vec<u8>], outputs: &mut Vec<&'a [u8]>) {
    outputs.clear();
    outputs.extend(buffers.iter().map(Vec::as_slice));
}

fn max_ulp_delta(reference: &[Vec<u8>], backend: &[Vec<u8>], program: &Program) -> Option<u32> {
    if reference.len() != backend.len() {
        return None;
    }
    let output_indices = program.output_buffer_indices();
    if reference.len() != output_indices.len() {
        return None;
    }
    let mut max_ulp = 0u32;
    for (slot, &buf_idx) in output_indices.iter().enumerate() {
        let bytes_a = reference.get(slot)?;
        let bytes_b = backend.get(slot)?;
        if program.buffers()[buf_idx as usize].element() != DataType::F32 {
            continue;
        }
        if bytes_a.len() != bytes_b.len() || bytes_a.len() % 4 != 0 {
            return None;
        }
        for (a, b) in bytes_a.chunks_exact(4).zip(bytes_b.chunks_exact(4)) {
            let fa = f32::from_bits(u32::from_le_bytes(a.try_into().unwrap()));
            let fb = f32::from_bits(u32::from_le_bytes(b.try_into().unwrap()));
            if fa.to_bits() == fb.to_bits() {
                continue;
            }
            if fa.is_nan() && fb.is_nan() {
                continue;
            }
            // Extreme inputs (inf, NaN) often diverge between CPU reference
            // and GPU due to fast-math / FTZ. Only same-signed infinities
            // and same-class non-finite values are comparable for ULP.
            if !fa.is_finite() && !fb.is_finite() {
                if fa.is_infinite()
                    && fb.is_infinite()
                    && fa.is_sign_positive() == fb.is_sign_positive()
                {
                    continue;
                }
                if fa.is_nan() && fb.is_nan() {
                    continue;
                }
                return Some(u32::MAX);
            }
            if fa.is_nan() || fb.is_nan() {
                return Some(u32::MAX);
            }
            match ulp_distance(fa, fb) {
                Some(ulp) => max_ulp = max_ulp.max(ulp),
                None => return Some(u32::MAX),
            }
        }
    }
    Some(max_ulp)
}

fn audit_f32_ulp_budget(program: &Program) -> u32 {
    if program_has_transcendental(program) {
        TRANSCENDENTAL_F32_ULP_BUDGET
    } else {
        f32_ulp_tolerance(program)
    }
}

fn program_has_transcendental(program: &Program) -> bool {
    program.entry().iter().any(node_has_transcendental)
}

fn expr_has_transcendental(expr: &Expr) -> bool {
    match expr {
        Expr::UnOp { op, operand } => {
            matches!(
                op,
                UnOp::Sqrt
                    | UnOp::InverseSqrt
                    | UnOp::Sin
                    | UnOp::Cos
                    | UnOp::Exp
                    | UnOp::Log
                    | UnOp::Log2
                    | UnOp::Exp2
                    | UnOp::Tan
                    | UnOp::Acos
                    | UnOp::Asin
                    | UnOp::Atan
                    | UnOp::Tanh
                    | UnOp::Sinh
                    | UnOp::Cosh
            ) || expr_has_transcendental(operand)
        }
        Expr::BinOp { left, right, .. } => {
            expr_has_transcendental(left) || expr_has_transcendental(right)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_has_transcendental(cond)
                || expr_has_transcendental(true_val)
                || expr_has_transcendental(false_val)
        }
        Expr::Cast { value, .. } => expr_has_transcendental(value),
        Expr::Fma { a, b, c } => {
            expr_has_transcendental(a) || expr_has_transcendental(b) || expr_has_transcendental(c)
        }
        Expr::Load { index, .. } => expr_has_transcendental(index),
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            expr_has_transcendental(index)
                || expected.as_deref().is_some_and(expr_has_transcendental)
                || expr_has_transcendental(value)
        }
        Expr::SubgroupReduce { value, .. } | Expr::SubgroupBallot { cond: value } => {
            expr_has_transcendental(value)
        }
        Expr::SubgroupShuffle { value, lane } => {
            expr_has_transcendental(value) || expr_has_transcendental(lane)
        }
        Expr::Call { args, .. } => args.iter().any(expr_has_transcendental),
        _ => false,
    }
}

fn node_has_transcendental(node: &Node) -> bool {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => expr_has_transcendental(value),
        Node::Store { index, value, .. } => {
            expr_has_transcendental(index) || expr_has_transcendental(value)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_has_transcendental(cond)
                || then.iter().any(node_has_transcendental)
                || otherwise.iter().any(node_has_transcendental)
        }
        Node::Loop { from, to, body, .. } => {
            expr_has_transcendental(from)
                || expr_has_transcendental(to)
                || body.iter().any(node_has_transcendental)
        }
        Node::Block(body) => body.iter().any(node_has_transcendental),
        Node::Region { body, .. } => body.iter().any(node_has_transcendental),
        _ => false,
    }
}

fn make_adversarial_inputs_into(
    base: &[Vec<u8>],
    program: &Program,
    input_indices: &[usize],
    value: f32,
    outputs: &mut Vec<Vec<u8>>,
) {
    if base.len() != input_indices.len() {
        panic!(
            "Fix: normalized adversarial input count {} does not match backend input count {}",
            base.len(),
            input_indices.len()
        );
    }
    outputs.clear();
    outputs.reserve(base.len());
    base.iter()
        .zip(input_indices.iter())
        .for_each(|(bytes, buffer_index)| {
            let decl = &program.buffers()[*buffer_index];
            let new = if decl.element() == DataType::F32 {
                let mut new = bytes.clone();
                assert_eq!(
                    new.len() % 4,
                    0,
                    "F32 buffer `{}` length {} not divisible by 4",
                    decl.name(),
                    new.len()
                );
                for chunk in new.chunks_exact_mut(4) {
                    chunk.copy_from_slice(&value.to_le_bytes());
                }
                new
            } else {
                bytes.clone()
            };
            outputs.push(new);
        });
}

const ADVERSARIAL_VALUES: &[f32] = &[
    1.0,
    -1.0,
    0.5,
    -0.5,
    2.0,
    -2.0,
    0.0,
    -0.0,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    f32::MIN_POSITIVE,
    f32::MAX,
    f32::from_bits(1),
    f32::from_bits(0x8000_0001),
    f32::from_bits(0x007f_ffff),
    f32::from_bits(0x807f_ffff),
];

fn adversarial_value_requires_ulp(value: f32) -> bool {
    value.is_finite() && value.abs() > f32::MIN_POSITIVE && value.abs() < f32::MAX
}

fn build_dispatch_backend() -> Box<dyn VyreBackend> {
    force_link_backend_inventory();
    let selected = std::env::var("VYRE_BACKEND")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let registration = vyre::backend::registered_backends()
        .iter()
        .find(|r| {
            vyre::backend::backend_dispatches(r.id)
                && selected
                    .as_deref()
                    .map_or(true, |backend| r.id == backend)
        })
        .expect(
            "Fix: a dispatch-capable backend must be registered for ULP audit. \
             Link a concrete driver crate into the test binary.",
        );
    registration.acquire().unwrap_or_else(|error| {
        panic!(
            "Fix: dispatch-capable backend `{}` failed its factory probe: {error}",
            registration.id
        )
    })
}

fn force_link_backend_inventory() {
    #[cfg(feature = "gpu")]
    {
        let metal_acquire: fn() -> Result<Box<dyn VyreBackend>, vyre_driver::backend::BackendError> =
            vyre_driver_metal::acquire;
        std::hint::black_box(metal_acquire);
    }
}

// ULP audit dispatches every registered op through a real dispatch-capable
// backend. Missing concrete GPU drivers must fail loudly instead of compiling
// this module out.
mod ulp_audit_part1 {

    include!("__split/ulp_audit_part1.rs");
}
