use super::*;
use vyre_foundation::ir::Program;

pub(super) struct DominatorDispatcher {
    pub(super) outputs: Vec<Vec<u8>>,
}

impl OptimizerDispatcher for DominatorDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        if inputs.len() != 6 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: dominator frontier test dispatcher expected 6 inputs, got {}.",
                inputs.len()
            )));
        }
        Ok(self.outputs.clone())
    }
}

pub(super) struct DominatorInputShapeDispatcher;

impl OptimizerDispatcher for DominatorInputShapeDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        assert_eq!(inputs.len(), 6);
        assert_eq!(
            inputs[1].len(),
            4,
            "Fix: empty dominance targets must be padded to one u32 from the primitive plan"
        );
        assert_eq!(
            inputs[3].len(),
            4,
            "Fix: empty predecessor targets must be padded to one u32 from the primitive plan"
        );
        Ok(vec![u32_slice_to_le_bytes(&[0])])
    }
}

pub(super) struct RecordingDominatorDispatcher {
    pub(super) calls: Mutex<Vec<Vec<Vec<u8>>>>,
    pub(super) output: Vec<u8>,
}

impl OptimizerDispatcher for RecordingDominatorDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        assert_eq!(grid_override, Some([1, 1, 1]));
        self.calls
            .lock()
            .expect("Fix: recording dispatcher calls lock should not be poisoned")
            .push(inputs.to_vec());
        Ok(vec![self.output.clone()])
    }
}
