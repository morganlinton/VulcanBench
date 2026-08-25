//! CUDA dispatch plan assembly helpers.

use smallvec::SmallVec;
use vyre_driver::binding::{Binding, BindingPlan};
use vyre_driver::BackendError;
use vyre_driver::LaunchPlan;

use super::ordering::sort_unstable_by_key_if_needed;
use super::staging_reserve::reserve_smallvec;

pub(crate) fn compute_ordered_output_indices(
    bindings: &BindingPlan,
) -> Result<SmallVec<[usize; 8]>, BackendError> {
    let mut output_indices = SmallVec::<[usize; 8]>::new();
    reserve_smallvec(
        &mut output_indices,
        bindings.output_indices.len(),
        "CUDA ordered output binding indices",
    )?;
    let mut last_output_index = None;
    let mut monotonic = true;
    for (binding_index, binding) in bindings.bindings.iter().enumerate() {
        if let Some(output_index) = binding.output_index {
            if let Some(last) = last_output_index {
                if output_index < last {
                    monotonic = false;
                    break;
                }
            }
            last_output_index = Some(output_index);
            output_indices.push(binding_index);
        }
    }
    if monotonic {
        return Ok(output_indices);
    }

    let mut ordered = SmallVec::<[(usize, usize); 8]>::new();
    reserve_smallvec(
        &mut ordered,
        bindings.output_indices.len(),
        "CUDA ordered output binding scratch",
    )?;
    for (binding_index, binding) in bindings.bindings.iter().enumerate() {
        if let Some(output_index) = binding.output_index {
            ordered.push((output_index, binding_index));
        }
    }
    sort_unstable_by_key_if_needed(&mut ordered, |(output_index, _)| *output_index);
    output_indices.clear();
    for (_, binding_index) in ordered {
        output_indices.push(binding_index);
    }
    Ok(output_indices)
}

#[derive(Debug, Clone)]
pub(crate) struct CudaDispatchPlan {
    pub(crate) bindings: BindingPlan,
    pub(crate) output_binding_indices: SmallVec<[usize; 8]>,
    pub(crate) launch: LaunchPlan,
    /// Mirrors `DispatchConfig::cooperative`; validated before launch.
    pub(crate) cooperative: bool,
    /// Mirrors `DispatchConfig::fixpoint_iterations`; the host-side
    /// dispatch loop runs the kernel this many times back-to-back on
    /// the same stream so that multi-hop dataflow primitives (the
    /// `flows_to`, `dominates`, `bounded_by_comparison` BFS-on-CSR
    /// chains in consumer rule lowerings) actually converge.
    /// Single-launch kernels read `1` as the conventional default. The
    /// Other GPU backends honor this same field via their persistent-pipeline
    /// fixpoint loops; the CUDA backend reached parity 2026-05-01.
    pub(crate) fixpoint_iterations: u32,
}

impl CudaDispatchPlan {
    pub(crate) fn output_binding(
        &self,
        binding_index: usize,
        context: &'static str,
    ) -> Result<&Binding, BackendError> {
        let Some(binding) = self.bindings.bindings.get(binding_index) else {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA {context} expected output binding index {binding_index}, but the dispatch plan only has {} binding descriptor(s). Rebuild the dispatch plan before launch.",
                    self.bindings.bindings.len()
                ),
            });
        };
        if binding.output_index.is_none() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA {context} resolved binding index {binding_index} to `{}` without an output index. Rebuild output binding ordering before launch.",
                    binding.name
                ),
            });
        }
        Ok(binding)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vyre_driver::binding::{Binding, BindingPlan, BindingRole};

    use super::*;

    fn output_binding(binding: u32, buffer_index: usize, output_index: usize) -> Binding {
        Binding {
            name: Arc::from(format!("out{binding}")),
            binding,
            buffer_index,
            role: BindingRole::Output,
            element_size: 4,
            preferred_alignment: 4,
            element_count: 1,
            static_byte_len: Some(4),
            input_index: None,
            output_index: Some(output_index),
        }
    }

    fn output_plan(output_indices: &[usize]) -> BindingPlan {
        let bindings = output_indices
            .iter()
            .enumerate()
            .map(|(binding_index, output_index)| {
                output_binding(binding_index as u32, binding_index, *output_index)
            })
            .collect();
        BindingPlan {
            bindings,
            input_indices: Vec::new(),
            output_indices: (0..output_indices.len()).collect(),
            shared_indices: Vec::new(),
        }
    }

    #[test]
    fn ordered_output_indices_keep_monotonic_binding_order_without_sorting() {
        let plan = output_plan(&[0, 1, 2, 3]);

        let ordered = compute_ordered_output_indices(&plan).unwrap();

        assert_eq!(ordered.as_slice(), &[0, 1, 2, 3]);
    }

    #[test]
    fn ordered_output_indices_sort_only_when_descriptors_are_out_of_order() {
        let plan = output_plan(&[2, 0, 3, 1]);

        let ordered = compute_ordered_output_indices(&plan).unwrap();

        assert_eq!(ordered.as_slice(), &[1, 3, 0, 2]);
    }

    #[test]
    fn ordered_output_indices_reserve_fallibly() {
        let source = include_str!("plan.rs");
        assert!(
            source.contains("use super::staging_reserve::reserve_smallvec;"),
            "Fix: CUDA dispatch-plan helpers must use the shared fallible staging reservation contract."
        );
        assert!(
            source.contains("\"CUDA ordered output binding scratch\"")
                && source.contains("\"CUDA ordered output binding indices\""),
            "Fix: CUDA output binding ordering must label both fallible scratch reservations."
        );
        assert!(
            source.contains("if monotonic {\n        return Ok(output_indices);\n    }"),
            "Fix: CUDA output binding ordering must keep the already-ordered path allocation-light and sort-free."
        );
        assert!(
            !source.contains(concat!(
                "SmallVec::<[(usize, usize); 8]>::",
                "with_capacity"
            )) && !source.contains(concat!("SmallVec::<[usize; 8]>::", "with_capacity")),
            "Fix: CUDA output binding ordering must not allocate scratch infallibly."
        );
    }

    #[test]
    fn output_binding_accessor_rejects_stale_or_non_output_indices() {
        let plan = CudaDispatchPlan {
            bindings: output_plan(&[0]),
            output_binding_indices: smallvec::smallvec![0],
            launch: LaunchPlan::new(),
            cooperative: false,
            fixpoint_iterations: 1,
        };

        assert_eq!(
            plan.output_binding(0, "test output")
                .expect("Fix: valid output binding must resolve.")
                .name
                .as_ref(),
            "out0"
        );
        assert!(
            plan.output_binding(1, "test output").is_err(),
            "Fix: stale output binding indexes must return BackendError instead of panicking."
        );

        let mut non_output_plan = plan.clone();
        non_output_plan.bindings.bindings[0].output_index = None;
        assert!(
            non_output_plan.output_binding(0, "test output").is_err(),
            "Fix: output binding indexes that resolve to non-output descriptors must return BackendError."
        );
    }
}
