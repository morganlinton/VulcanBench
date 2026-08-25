use super::*;

pub(crate) fn resident_in_place_reference_outputs(
    backend: &CudaBackend,
    program: &Program,
    inputs: &[Vec<u8>],
    readback_indices: &[usize],
    case_name: &str,
) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut handles = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        let handle = backend.allocate_resident(input.len()).unwrap_or_else(|error| {
            panic!(
                "Fix: CUDA resident in-place generated case `{case_name}` input {index} allocation failed: {error}"
            )
        });
        backend.upload_resident(handle, input).unwrap_or_else(|error| {
            panic!(
                "Fix: CUDA resident in-place generated case `{case_name}` input {index} upload failed: {error}"
            )
        });
        handles.push(handle);
    }
    backend
        .dispatch_resident(program, &handles, &Default::default())
        .unwrap_or_else(|error| {
            panic!(
                "Fix: CUDA resident in-place generated case `{case_name}` dispatch failed: {error}"
            )
        });

    let mut resident_cuda = Vec::with_capacity(readback_indices.len());
    for &index in readback_indices {
        resident_cuda.push(backend.download_resident(handles[index]).unwrap_or_else(|error| {
            panic!(
                "Fix: CUDA resident in-place generated case `{case_name}` readback {index} failed: {error}"
            )
        }));
    }
    for handle in handles {
        backend.free_resident(handle).unwrap_or_else(|error| {
            panic!(
                "Fix: CUDA resident in-place generated case `{case_name}` cleanup failed: {error}"
            )
        });
    }
    let reference = reference_outputs(program, inputs, case_name);
    (resident_cuda, reference)
}

