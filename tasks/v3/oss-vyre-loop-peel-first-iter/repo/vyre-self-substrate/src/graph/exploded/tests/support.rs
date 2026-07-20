use super::*;
use vyre_foundation::ir::Program;

pub(super) fn canonical_expected(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra: &[(u32, u32, u32)],
    inter: &[(u32, u32, u32, u32)],
    gen_edges: &[(u32, u32, u32)],
    kill: &[(u32, u32, u32)],
) -> (Vec<u32>, Vec<u32>) {
    let (row_ptr, col_idx) = reference_build_ifds_csr(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra,
        inter,
        gen_edges,
        kill,
    );
    reference_canonicalize_csr_within_rows(&row_ptr, &col_idx)
}

pub(super) struct RecordingIfdsOracle {
    pub(super) inner: CpuOracleDispatcher,
    pub(super) intra_src_blocks: Mutex<Vec<Vec<u32>>>,
}

impl OptimizerDispatcher for RecordingIfdsOracle {
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        if inputs.len() >= 2 {
            self.intra_src_blocks
                .lock()
                .expect("Fix: IFDS recording mutex should not be poisoned")
                .push(crate::hardware::dispatch_buffers::read_u32s(&inputs[1]));
        }
        self.inner.dispatch(program, inputs, grid_override)
    }
}

/// Test-only dispatcher that returns malformed backend outputs for decode
/// validation. Production parity uses [`CpuOracleDispatcher`].
pub(super) struct MalformedIfdsDispatcher {
    pub(super) outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for MalformedIfdsDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Ok(self.outputs.clone())
    }
}
