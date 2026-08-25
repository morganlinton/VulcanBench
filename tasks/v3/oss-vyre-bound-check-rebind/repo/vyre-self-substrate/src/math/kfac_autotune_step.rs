//! K-FAC step inside vyre's natural-gradient autotuner.
//!
//! Replaces standard gradient descent on dispatch-graph continuous
//! variables (e.g. tile sizes, fusion probabilities) with Fisher-
//! preconditioned updates.
//!
//! Dispatches the `vyre_primitives::math::kfac_block_inverse` primitive
//! to invert the block-diagonal Fisher information matrix of the
//! autotuner's policy network.

use vyre_foundation::ir::Program;
use vyre_primitives::math::kfac_block_inverse::kfac_block_inverse;

use crate::dispatch_buffers::{
    ceil_div_u32, decode_f32_output_exact, ensure_input_slots, write_f32_slice_le_bytes,
    write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Canonical op ID for the autotune step.
pub const OP_ID: &str = "vyre-libs::self_substrate::kfac_autotune_step";

/// Caller-owned GPU dispatch scratch for K-FAC autotune steps.
#[derive(Debug, Default)]
pub struct KfacAutotuneGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Compile a Program that inverts the Fisher block-diagonal matrix.
///
/// `n` is the size of each block (e.g. number of parameters in a layer).
/// `num_blocks` is the number of independent layers/blocks.
#[must_use]
pub fn kfac_autotune_step_program(
    blocks_out: &str,
    blocks_in: &str,
    scratch: &str,
    num_blocks: u32,
    n: u32,
) -> Program {
    use crate::observability::{bump, kfac_autotune_step_calls};
    bump(&kfac_autotune_step_calls);
    kfac_block_inverse(blocks_out, blocks_in, scratch, num_blocks, n)
}

/// GPU dispatch wrapper around [`kfac_autotune_step_program`].
/// Returns the inverted block-diagonal Fisher matrix for the supplied
/// blocks.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed dimensions or readback.
pub fn kfac_autotune_step_via(
    dispatcher: &dyn OptimizerDispatcher,
    blocks_in: &[f32],
    num_blocks: u32,
    n: u32,
) -> Result<Vec<f32>, DispatchError> {
    let mut out = Vec::new();
    kfac_autotune_step_via_into(dispatcher, blocks_in, num_blocks, n, &mut out)?;
    Ok(out)
}

/// GPU dispatch wrapper around [`kfac_autotune_step_program`] into caller-owned
/// output storage.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed dimensions or readback.
pub fn kfac_autotune_step_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    blocks_in: &[f32],
    num_blocks: u32,
    n: u32,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    let mut scratch = KfacAutotuneGpuScratch::default();
    kfac_autotune_step_via_with_scratch_into(
        dispatcher,
        blocks_in,
        num_blocks,
        n,
        &mut scratch,
        out,
    )
}

/// GPU dispatch wrapper around [`kfac_autotune_step_program`] into
/// caller-owned dispatch and output storage.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed dimensions or readback.
pub fn kfac_autotune_step_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    blocks_in: &[f32],
    num_blocks: u32,
    n: u32,
    scratch: &mut KfacAutotuneGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    if num_blocks == 0 || n == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: kfac_autotune_step_via requires num_blocks > 0 and n > 0; got num_blocks={num_blocks}, n={n}."
        )));
    }
    let block_cells = n.checked_mul(n).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: kfac_autotune_step_via block size overflows n*n for n={n}."
        ))
    })?;
    let total_cells = num_blocks.checked_mul(block_cells).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: kfac_autotune_step_via total size overflows num_blocks*n*n for num_blocks={num_blocks}, n={n}."
        ))
    })? as usize;
    if blocks_in.len() != total_cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: kfac_autotune_step_via expected blocks_in.len() == num_blocks*n*n == {total_cells}, got {}.",
            blocks_in.len()
        )));
    }

    let program = kfac_autotune_step_program("blocks_out", "blocks_in", "scratch", num_blocks, n);
    let byte_len = total_cells
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: kfac_autotune_step_via byte size overflows usize for {total_cells} cells."
            ))
        })?;
    ensure_input_slots(&mut scratch.inputs, 3);
    write_zero_bytes(&mut scratch.inputs[0], byte_len);
    write_f32_slice_le_bytes(&mut scratch.inputs[1], blocks_in);
    write_zero_bytes(&mut scratch.inputs[2], byte_len);
    let grid_x = ceil_div_u32(num_blocks, 256);
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([grid_x, 1, 1]))?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: kfac_autotune_step_via expected at least the blocks_out output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_f32_output_exact(&outputs[0], total_cells, "kfac_autotune_step_via", out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::f32_slice_to_le_bytes;
    use vyre_primitives::math::kfac_block_inverse::cpu_ref;

    struct KfacDispatcher;

    impl OptimizerDispatcher for KfacDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 3);
            assert_eq!(inputs[0].len(), inputs[1].len());
            assert_eq!(inputs[2].len(), inputs[1].len());
            let blocks_in = crate::hardware::dispatch_buffers::read_f32s(&inputs[1]);
            let out = cpu_ref(&blocks_in, 1, 2);
            Ok(vec![f32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn test_kfac_program_shape() {
        let p = kfac_autotune_step_program("bo", "bi", "s", 10, 16);
        assert_eq!(p.buffers().len(), 3, "Expects exactly 3 buffers");
        assert!(p.buffers().iter().any(|b| b.name() == "bi"));
    }

    #[test]
    fn test_kfac_autotune_fisher_block() {
        // Non-trivial vyre IR shape: 2 blocks of size 2x2.
        // Block 1: Identity
        // Block 2: Diagonal [2, 4] -> inverse is [0.5, 0.25]
        let num_blocks = 2;
        let n = 2;
        let blocks_in = vec![
            1.0, 0.0, 0.0, 1.0, // block 0
            2.0, 0.0, 0.0, 4.0, // block 1
        ];

        let out = cpu_ref(&blocks_in, num_blocks, n);

        assert_eq!(out[0..4], vec![1.0, 0.0, 0.0, 1.0]);
        assert_eq!(out[4..8], vec![0.5, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn test_kfac_autotune_dense_block() {
        // Dense block
        let num_blocks = 1;
        let n = 2;
        let blocks_in = vec![4.0, 3.0, 3.0, 2.0];
        // determinant = 4*2 - 3*3 = 8 - 9 = -1
        // inverse = [-2, 3; 3, -4]

        let out = cpu_ref(&blocks_in, num_blocks, n);

        assert_eq!(out, vec![-2.0, 3.0, 3.0, -4.0]);
    }

    #[test]
    fn test_multi_layer_kfac_composition() {
        let p1 = kfac_autotune_step_program("bo1", "bi1", "s1", 1, 4);
        let p2 = kfac_autotune_step_program("bo2", "bi2", "s2", 1, 4);
        let p3 = kfac_autotune_step_program("bo3", "bi3", "s3", 1, 4);

        let final_p = crate::test_support::wrap_program_sequence(&[&p1, &p2, &p3], [256, 1, 1]);
        let region_count = final_p
            .entry()
            .iter()
            .filter(|n| matches!(n, vyre_foundation::ir::Node::Region { .. }))
            .count();
        assert!(region_count >= 3);
    }

    #[test]
    fn test_end_to_end_kfac_parity() {
        let blocks_in = vec![2.0, 0.0, 0.0, 4.0];
        let p = kfac_autotune_step_program("bo", "bi", "s", 1, 2);

        use std::sync::Arc;
        use vyre_reference::reference_eval;
        use vyre_reference::value::Value;

        let to_value = |data: &[f32]| {
            let bytes = vyre_primitives::wire::pack_f32_slice(data);
            Value::Bytes(Arc::from(bytes))
        };

        let inputs = vec![
            to_value(&[0.0; 4]),
            to_value(&blocks_in),
            to_value(&[0.0; 4]),
        ];

        let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
        let actual_bytes = results[0].to_bytes();
        let actual_out: Vec<f32> = actual_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(actual_out, vec![0.5, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn kfac_autotune_step_via_dispatches_primitive() {
        let blocks_in = vec![2.0, 0.0, 0.0, 4.0];

        let out = kfac_autotune_step_via(&KfacDispatcher, &blocks_in, 1, 2).unwrap();

        assert_eq!(out, vec![0.5, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn kfac_autotune_step_via_into_reuses_output() {
        let blocks_in = vec![2.0, 0.0, 0.0, 4.0];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();

        kfac_autotune_step_via_into(&KfacDispatcher, &blocks_in, 1, 2, &mut out).unwrap();

        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out, vec![0.5, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn kfac_autotune_step_via_with_scratch_reuses_dispatch_and_output_storage() {
        let blocks_in = vec![2.0, 0.0, 0.0, 4.0];
        let mut scratch = KfacAutotuneGpuScratch::default();
        let mut out = Vec::with_capacity(4);

        kfac_autotune_step_via_with_scratch_into(
            &KfacDispatcher,
            &blocks_in,
            1,
            2,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let out_capacity = out.capacity();

        kfac_autotune_step_via_with_scratch_into(
            &KfacDispatcher,
            &blocks_in,
            1,
            2,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(out, vec![0.5, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn kfac_autotune_step_via_rejects_bad_shape() {
        let err = kfac_autotune_step_via(&KfacDispatcher, &[1.0, 0.0], 1, 2).unwrap_err();

        assert!(matches!(err, DispatchError::BadInputs(_)));
    }
}
