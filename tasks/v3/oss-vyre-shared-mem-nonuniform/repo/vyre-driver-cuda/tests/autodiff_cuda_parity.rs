//! Live CUDA/reference parity for autodiff-generated backward Programs.

mod common;

use common::bytes_u32;
use common::{bytes_f32, cuda_reference_outputs, f32_bytes, live_backend};
use vyre_driver::binding::BindingPlan;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::transform::autodiff::grad;

const LANES: usize = 64;

#[test]
fn cuda_preserves_final_store_when_same_lane_is_written_twice() {
    let backend = live_backend();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(8)],
        [64, 1, 1],
        vec![
            Node::store("out", Expr::gid_x(), Expr::u32(0)),
            Node::store("out", Expr::gid_x(), Expr::u32(1)),
        ],
    );
    let outputs = cuda_reference_outputs(&backend, &program, &[], "double_store");
    for (path, buffers) in [
        ("direct CUDA", &outputs.direct_cuda),
        ("compiled CUDA", &outputs.compiled_cuda),
        ("reference", &outputs.reference),
    ] {
        assert_eq!(
            bytes_u32(&buffers[0]),
            vec![1; 8],
            "Fix: {path} must preserve statement order for repeated stores to the same output lane."
        );
    }
}

#[test]
fn cuda_store_then_load_from_backend_allocated_output_is_visible_in_kernel() {
    let backend = live_backend();
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("scratch", 0, DataType::U32)
                .with_count(8)
                .with_pipeline_live_out(true),
            BufferDecl::output("out", 1, DataType::U32).with_count(8),
        ],
        [64, 1, 1],
        vec![
            Node::store("scratch", Expr::gid_x(), Expr::u32(7)),
            Node::store("out", Expr::gid_x(), Expr::load("scratch", Expr::gid_x())),
        ],
    );
    let outputs = cuda_reference_outputs(&backend, &program, &[], "store_then_load_output");
    for (path, buffers) in [
        ("direct CUDA", &outputs.direct_cuda),
        ("compiled CUDA", &outputs.compiled_cuda),
        ("reference", &outputs.reference),
    ] {
        assert_eq!(
            bytes_u32(&buffers[0]),
            vec![7; 8],
            "Fix: {path} must retain the backend-allocated scratch output store."
        );
        assert_eq!(
            bytes_u32(&buffers[1]),
            vec![7; 8],
            "Fix: {path} must make a same-kernel output store visible to a later load."
        );
    }
}

#[test]
fn cuda_executes_autodiff_generated_fma_square_backward_program() {
    let backend = live_backend();
    let forward = differentiable_fma_square_program(LANES as u32);
    let backward = grad(&forward, &["out"], &["x", "w"])
        .expect("Fix: fma-square program must lower through reverse-mode autodiff.");
    let live_outs = backward
        .buffers()
        .iter()
        .filter(|buffer| buffer.is_pipeline_live_out())
        .map(|buffer| buffer.name())
        .collect::<Vec<_>>();
    assert_eq!(
        live_outs,
        vec!["grad_out", "grad_x", "grad_w"],
        "Fix: autodiff CUDA parity assumes stable live-out gradient ordering."
    );

    let x = generated_x_lanes();
    let w = generated_w_lanes();
    let out = x
        .iter()
        .zip(&w)
        .map(|(&x, &w)| w.mul_add(x, x * x))
        .collect::<Vec<_>>();
    let inputs = vec![f32_bytes(&x), f32_bytes(&w), f32_bytes(&out)];
    let outputs = cuda_reference_outputs(&backend, &backward, &inputs, "autodiff_fma_square");

    // The output seed is internal adjoint scratch: the Store pullback reads
    // grad_out and then clears it so repeated backward dispatches do not leak
    // stale seed state.
    let expected_grad_out = vec![0.0f32; LANES];
    let expected_grad_x = x
        .iter()
        .zip(&w)
        .map(|(&x, &w)| w + (2.0 * x))
        .collect::<Vec<_>>();
    let expected_grad_w = x.clone();

    let output_plan = BindingPlan::from_program(&backward, &inputs)
        .expect("Fix: autodiff backward program must produce a valid CUDA binding plan.");
    for (path, buffers) in [
        ("reference", &outputs.reference),
        ("direct CUDA", &outputs.direct_cuda),
        ("compiled CUDA", &outputs.compiled_cuda),
    ] {
        assert_autodiff_outputs_by_binding(
            path,
            &output_plan,
            buffers,
            &expected_grad_out,
            &expected_grad_x,
            &expected_grad_w,
        );
    }
}

fn differentiable_fma_square_program(lanes: u32) -> Program {
    let idx = Expr::gid_x();
    let x = Expr::load("x", idx.clone());
    let w = Expr::load("w", idx.clone());
    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(lanes),
            BufferDecl::storage("w", 1, BufferAccess::ReadOnly, DataType::F32).with_count(lanes),
            BufferDecl::output("out", 2, DataType::F32).with_count(lanes),
        ],
        [128, 1, 1],
        vec![
            Node::let_bind("xx", Expr::mul(x.clone(), x)),
            Node::let_bind(
                "y",
                Expr::fma(w, Expr::load("x", idx.clone()), Expr::var("xx")),
            ),
            Node::store("out", idx, Expr::var("y")),
        ],
    )
}

fn generated_x_lanes() -> Vec<f32> {
    (0..LANES)
        .map(|lane| match lane % 16 {
            0 => 0.0,
            1 => 1.0,
            2 => -1.0,
            3 => 2.0,
            4 => -2.0,
            5 => 8.0,
            6 => -8.0,
            7 => 0.5,
            8 => -0.5,
            _ => (lane as f32 - 31.0) * 0.25,
        })
        .collect()
}

fn generated_w_lanes() -> Vec<f32> {
    (0..LANES)
        .map(|lane| match lane % 12 {
            0 => 0.0,
            1 => 3.0,
            2 => -3.0,
            3 => 0.25,
            4 => -0.25,
            _ => (lane as f32 % 9.0) - 4.0,
        })
        .collect()
}

fn assert_f32_bits_eq(path: &str, buffer: &str, actual: &[f32], expected: &[f32]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "Fix: {path} {buffer} lane count changed."
    );
    for (lane, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "Fix: {path} {buffer}[{lane}] diverged from analytical autodiff gradient: actual={actual:?} expected={expected:?}."
        );
    }
}

fn assert_autodiff_outputs_by_binding(
    path: &str,
    output_plan: &BindingPlan,
    buffers: &[Vec<u8>],
    expected_grad_out: &[f32],
    expected_grad_x: &[f32],
    expected_grad_w: &[f32],
) {
    assert_eq!(
        buffers.len(),
        output_plan.output_indices.len(),
        "Fix: {path} autodiff backward dispatch output count diverged from the binding plan."
    );
    let mut checked = 0usize;
    for binding in &output_plan.bindings {
        let Some(output_index) = binding.output_index else {
            continue;
        };
        let expected = match binding.name.as_ref() {
            "grad_out" => expected_grad_out,
            "grad_x" => expected_grad_x,
            "grad_w" => expected_grad_w,
            other => panic!(
                "Fix: {path} autodiff backward dispatch returned unexpected gradient output `{other}`."
            ),
        };
        assert_f32_bits_eq(
            path,
            binding.name.as_ref(),
            &bytes_f32(&buffers[output_index]),
            expected,
        );
        checked += 1;
    }
    assert_eq!(
        checked, 3,
        "Fix: {path} autodiff backward dispatch must expose grad_out, grad_x, and grad_w."
    );
}
