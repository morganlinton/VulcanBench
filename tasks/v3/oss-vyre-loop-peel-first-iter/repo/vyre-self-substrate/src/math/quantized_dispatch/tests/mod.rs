mod batched_matmul_contracts;
mod batched_matmul_top1_contracts;
mod batched_matvec_contracts;
mod dot_contracts;
mod generated_contracts;
mod matvec_contracts;
mod unpack_contracts;

use super::*;

struct QuantizedDispatcher;

impl OptimizerDispatcher for QuantizedDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        assert_eq!(inputs.len(), 2);
        let packed = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let lane_count = inputs[1].len() / std::mem::size_of::<i32>();
        let mut out = Vec::new();
        unpack_i4x8_cpu_into(&packed, lane_count as u32, &mut out);
        Ok(vec![vyre_primitives::wire::pack_i32_slice(&out)])
    }
}

struct QuantizedDotDispatcher;

impl OptimizerDispatcher for QuantizedDotDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        assert_eq!(inputs.len(), 5);
        let lhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let rhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let lhs_scale = crate::hardware::dispatch_buffers::read_f32s(&inputs[2])[0];
        let rhs_scale = crate::hardware::dispatch_buffers::read_f32s(&inputs[3])[0];
        let lane_count = (inputs[4].len() / std::mem::size_of::<f32>()) as u32;
        assert_eq!(
            lane_count, 1,
            "Fix: dot output slot must reserve exactly one f32 word."
        );
        let logical_lane_count = (lhs.len() as u32 - 1) * 8
            + if lhs.last().copied().unwrap_or(0) == 0 {
                8
            } else {
                8
            };
        let lane_count = logical_lane_count.min((lhs.len() as u32) * 8);
        let out = i4x8_dot_f32_scaled_cpu(&lhs, &rhs, lhs_scale, rhs_scale, lane_count);
        Ok(vec![vyre_primitives::wire::pack_f32_slice(&[out])])
    }
}

struct MalformedDotDispatcher {
    outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for MalformedDotDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Ok(self.outputs.clone())
    }
}

struct QuantizedMatvecDispatcher;

impl OptimizerDispatcher for QuantizedMatvecDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(inputs.len(), 4);
        let weights = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let x = crate::hardware::dispatch_buffers::read_f32s(&inputs[1]);
        let row_scales = crate::hardware::dispatch_buffers::read_f32s(&inputs[2]);
        let rows = row_scales.len() as u32;
        let cols = x.len() as u32;
        assert_eq!(grid_override, Some([rows, 1, 1]));
        assert_eq!(
            inputs[3].len(),
            row_scales.len() * std::mem::size_of::<f32>(),
            "Fix: matvec output slot must reserve exactly one f32 per row."
        );
        let out = i4x8_matvec_f32_scaled_cpu(&weights, &x, &row_scales, rows, cols);
        Ok(vec![vyre_primitives::wire::pack_f32_slice(&out)])
    }
}

struct QuantizedBatchedMatvecDispatcher;

impl OptimizerDispatcher for QuantizedBatchedMatvecDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(inputs.len(), 4);
        let weights = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let x_batches = crate::hardware::dispatch_buffers::read_f32s(&inputs[1]);
        let row_scales = crate::hardware::dispatch_buffers::read_f32s(&inputs[2]);
        let Some([rows, batch, 1]) = grid_override else {
            panic!("Fix: batched matvec dispatch must launch with [rows, batch, 1].");
        };
        let cols = x_batches
            .len()
            .checked_div(batch as usize)
            .expect("Fix: fake batched matvec dispatcher requires nonzero batch")
            as u32;
        assert_eq!(rows as usize, row_scales.len());
        assert_eq!(
            inputs[3].len(),
            batch as usize * rows as usize * std::mem::size_of::<f32>(),
            "Fix: batched matvec output slot must reserve exactly one f32 per batch row."
        );
        let out = i4x8_batched_matvec_f32_scaled_cpu(
            &weights,
            &x_batches,
            &row_scales,
            batch,
            rows,
            cols,
        );
        Ok(vec![vyre_primitives::wire::pack_f32_slice(&out)])
    }
}

struct QuantizedBatchedMatmulDispatcher;

impl OptimizerDispatcher for QuantizedBatchedMatmulDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(inputs.len(), 5);
        let weights = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let activations = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let row_scales = crate::hardware::dispatch_buffers::read_f32s(&inputs[2]);
        let batch_scales = crate::hardware::dispatch_buffers::read_f32s(&inputs[3]);
        let rows = row_scales.len() as u32;
        let batch = batch_scales.len() as u32;
        let Some([grid_x, 1, 1]) = grid_override else {
            panic!(
                "Fix: batched matmul dispatch must launch one-dimensional 64-wide workgroup grid."
            );
        };
        assert_eq!(grid_x, ceil_div_u32(batch * rows, 64));
        assert_eq!(
            inputs[4].len(),
            batch as usize * rows as usize * std::mem::size_of::<f32>(),
            "Fix: batched matmul output slot must reserve exactly one f32 per batch row."
        );
        let words_per_activation = activations.len() / batch as usize;
        let cols = (words_per_activation as u32) * 8;
        let out = i4x8_batched_matmul_f32_scaled_cpu(
            &weights,
            &activations,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );
        Ok(vec![vyre_primitives::wire::pack_f32_slice(&out)])
    }
}

struct QuantizedBatchedMatmulTop1Dispatcher;

impl OptimizerDispatcher for QuantizedBatchedMatmulTop1Dispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(inputs.len(), 6);
        let weights = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let activations = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let row_scales = crate::hardware::dispatch_buffers::read_f32s(&inputs[2]);
        let batch_scales = crate::hardware::dispatch_buffers::read_f32s(&inputs[3]);
        let rows = row_scales.len() as u32;
        let batch = batch_scales.len() as u32;
        assert_eq!(grid_override, Some([ceil_div_u32(batch, 64), 1, 1]));
        assert_eq!(
            inputs[4].len(),
            batch as usize * std::mem::size_of::<f32>(),
            "Fix: top-1 score output slot must reserve exactly one f32 per batch."
        );
        assert_eq!(
            inputs[5].len(),
            batch as usize * std::mem::size_of::<u32>(),
            "Fix: top-1 index output slot must reserve exactly one u32 per batch."
        );
        let words_per_activation = activations.len() / batch as usize;
        let cols = (words_per_activation as u32) * 8;
        let (scores, indices) = i4x8_batched_matmul_top1_f32_scaled_cpu(
            &weights,
            &activations,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );
        Ok(vec![
            vyre_primitives::wire::pack_f32_slice(&scores),
            vyre_primitives::wire::pack_u32_slice(&indices),
        ])
    }
}

fn pack_i4_rows(rows: &[&[i32]]) -> Vec<u32> {
    let mut packed = Vec::new();
    for row in rows {
        packed.extend(pack_i4x8_cpu(row));
    }
    packed
}

