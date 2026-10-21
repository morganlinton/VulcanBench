//! MuonEq-R: Row-normalized Muon optimizer (F32).
//!
//! Muon + `scale = max(1, rows/cols)^0.5` row normalization.

use vyre::ir::Program;

use crate::nn::optim::muon_core::muon_step_program;

const OP_ID: &str = "vyre-libs::optim::muoneq_r";

/// MuonEq-R step (F32).
///
/// Same as `muon_update` but with row-norm scaling baked in.
#[must_use]
pub fn muoneq_r(
    params: &str,
    grads: &str,
    momentum_buf: &str,
    output: &str,
    n: u32,
    rows: u32,
    cols: u32,
    lr: f32,
    momentum: f32,
) -> Program {
    // scale = max(1, rows/cols)^0.5
    let ratio = (rows as f32) / (cols as f32);
    let scale = ratio.max(1.0).sqrt();
    muon_step_program(
        OP_ID,
        params,
        grads,
        momentum_buf,
        output,
        n,
        scale * lr,
        momentum,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || muoneq_r("params", "grads", "momentum", "output", 4, 4, 2, 0.02, 0.95),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0, 3.0, 4.0]),
                to_f32(&[0.1, 0.2, 0.3, 0.4]),
                to_f32(&[0.0, 0.0, 0.0, 0.0]),
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![
                vec![
                    205, 204, 204, 61, 205, 204, 76, 62, 154, 153, 153, 62, 205, 204, 204, 62,
                ],
                vec![
                    64, 239, 125, 63, 64, 239, 253, 63, 112, 115, 62, 64, 64, 239, 125, 64,
                ],
            ]]
        }),
        category: Some("nn"),
    }
}
