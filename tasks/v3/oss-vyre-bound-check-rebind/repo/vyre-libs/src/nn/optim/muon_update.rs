//! Muon update: momentum + Newton-Schulz orthogonalization (F32).
//!
//! `buf = momentum * buf + grad`
//! `nesterov = grad + momentum * buf`
//! `orthogonal = newton_schulz_5step(nesterov)` (via composition)
//! `param -= lr * orthogonal * scale`

use vyre::ir::Program;

use crate::nn::optim::muon_core::muon_step_program;

const OP_ID: &str = "vyre-libs::optim::muon_update";

/// Muon optimizer step (F32).
///
/// `params[n]` (RO), `grads[n]` (RO), `momentum_buf[n]` (RW),
/// `output[n]`  -  updated params.
#[must_use]
pub fn muon_update(
    params: &str,
    grads: &str,
    momentum_buf: &str,
    output: &str,
    n: u32,
    lr: f32,
    momentum: f32,
) -> Program {
    muon_step_program(OP_ID, params, grads, momentum_buf, output, n, lr, momentum)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || muon_update("params", "grads", "momentum", "output", 2, 0.02, 0.95),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0]),    // params
                to_f32(&[0.1, 0.2]),    // grads
                to_f32(&[0.0, 0.0]),    // momentum (first step)
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![
                vec![205, 204, 204, 61, 205, 204, 76, 62],
                vec![30, 138, 126, 63, 30, 138, 254, 63],
            ]]
        }),
        category: Some("nn"),
    }
}
