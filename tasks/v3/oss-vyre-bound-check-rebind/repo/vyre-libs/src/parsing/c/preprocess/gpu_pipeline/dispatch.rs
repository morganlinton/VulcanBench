use vyre::ir::Program;
use vyre::{DispatchConfig, VyreBackend};

/// Dispatcher abstraction: anything that can take a `Program` and
/// input buffers and return output buffers. Lets the orchestrator be
/// driven by either a real `VyreBackend` (production) or a closure
/// over `vyre_reference::reference_eval` (tests). The closure form
/// matters because `VyreBackend` is sealed  -  third-party impls aren't
/// allowed  -  and because the reference path needs none of the GPU
/// driver's transitive dependencies.
pub trait GpuDispatcher {
    /// Run `program` with `inputs`; return one `Vec<u8>` per output buffer.
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String>;

    /// Run `program` with borrowed input buffers. Default delegates to
    /// `dispatch` after staging the borrowed slices into owned `Vec<u8>`s,
    /// preserving behavior for dispatchers that only implement the owned
    /// path (notably the reference interpreter). Real-GPU dispatchers
    /// override this to forward the slices straight to
    /// `VyreBackend::dispatch_borrowed`, eliminating one full input-buffer
    /// clone per dispatch  -  material at the per-include preprocess hot
    /// loop where each translation unit fans out to ~30 dispatches.
    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        let owned: Vec<Vec<u8>> = inputs.iter().map(|s| s.to_vec()).collect();
        GpuDispatcher::dispatch(self, program, &owned)
    }

    /// Run `program` with borrowed input buffers and write backend outputs into
    /// caller-owned slots.
    ///
    /// The default delegates to [`GpuDispatcher::dispatch_borrowed`] and then
    /// moves each returned buffer into `outputs` while preserving existing slot
    /// allocations when possible. Real GPU backends override this to forward to
    /// `VyreBackend::dispatch_borrowed_into`, eliminating the returned
    /// `Vec<Vec<u8>>` allocation on hot preprocessing loops.
    fn dispatch_borrowed_into(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), String> {
        let result = self.dispatch_borrowed(program, inputs)?;
        replace_outputs_preserving_slots(outputs, result);
        Ok(())
    }

    /// Whether this dispatcher requires write-only/output buffers to be supplied
    /// as input values. Real GPU backends allocate declared outputs themselves;
    /// the reference interpreter consumes one value per non-workgroup buffer and
    /// therefore needs zero-initialized output buffers supplied explicitly.
    fn requires_output_inputs(&self) -> bool {
        false
    }
}

/// Adapter so any `&dyn VyreBackend` plugs into the orchestrator
/// without callers wrapping it manually.
pub struct BackendDispatcher<'a>(pub &'a dyn VyreBackend);

impl GpuDispatcher for BackendDispatcher<'_> {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        VyreBackend::dispatch(self.0, program, inputs, &DispatchConfig::default())
            .map_err(|e| format!("backend dispatch: {e}"))
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        self.0
            .dispatch_borrowed(program, inputs, &DispatchConfig::default())
            .map_err(|e| format!("backend dispatch_borrowed: {e}"))
    }

    fn dispatch_borrowed_into(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), String> {
        self.0
            .dispatch_borrowed_into(program, inputs, &DispatchConfig::default(), outputs)
            .map_err(|e| format!("backend dispatch_borrowed_into: {e}"))
    }
}

fn replace_outputs_preserving_slots(outputs: &mut Vec<Vec<u8>>, result: Vec<Vec<u8>>) {
    let mut incoming = result.into_iter();
    let mut reused = 0usize;
    for (slot, mut next) in outputs.iter_mut().zip(incoming.by_ref()) {
        if next.len() <= slot.capacity() {
            slot.clear();
            slot.extend_from_slice(&next);
        } else {
            std::mem::swap(slot, &mut next);
        }
        reused += 1;
    }
    outputs.truncate(reused);
    outputs.extend(incoming);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_outputs_preserving_slots_reuses_retained_buffers() {
        let mut outputs = vec![Vec::with_capacity(16), Vec::with_capacity(8)];
        outputs[0].extend_from_slice(&[9, 9]);
        outputs[1].extend_from_slice(&[8]);
        let outer_ptr = outputs.as_ptr() as usize;
        let first_ptr = outputs[0].as_ptr() as usize;
        let second_ptr = outputs[1].as_ptr() as usize;

        replace_outputs_preserving_slots(&mut outputs, vec![vec![1, 2, 3], vec![4]]);

        assert_eq!(outputs, vec![vec![1, 2, 3], vec![4]]);
        assert_eq!(outputs.as_ptr() as usize, outer_ptr);
        assert_eq!(outputs[0].as_ptr() as usize, first_ptr);
        assert_eq!(outputs[1].as_ptr() as usize, second_ptr);
    }

    #[test]
    fn replace_outputs_preserving_slots_moves_oversized_buffers() {
        let mut outputs = vec![Vec::with_capacity(1)];
        outputs[0].push(9);
        let incoming = vec![vec![1, 2, 3, 4]];
        let incoming_ptr = incoming[0].as_ptr() as usize;

        replace_outputs_preserving_slots(&mut outputs, incoming);

        assert_eq!(outputs, vec![vec![1, 2, 3, 4]]);
        assert_eq!(
            outputs[0].as_ptr() as usize,
            incoming_ptr,
            "Fix: oversized C-preprocessor GPU outputs must be moved into retained slots instead of copied through too-small buffers."
        );
    }
}
