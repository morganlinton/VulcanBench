//! Integration test for the CUDA backend.

use vyre_driver_cuda::CudaBackend;

#[path = "resident_buffer_contracts/range_readback_contracts.rs"]
mod range_readback_contracts;

#[test]
fn resident_buffer_round_trips_bytes_without_dispatch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");
    let output = backend
        .download_resident(handle)
        .expect("Fix: CUDA resident download failed.");
    assert_eq!(output, input);

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_allocated_bytes_tracks_allocate_and_free() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    assert_eq!(
        backend.resident_allocated_bytes(),
        0,
        "Fix: fresh CUDA backend must start with zero resident bytes."
    );

    let first = backend
        .allocate_resident(8)
        .expect("Fix: first CUDA resident buffer allocation failed.");
    assert_eq!(
        backend.resident_allocated_bytes(),
        8,
        "Fix: resident byte accounting must include the first live handle."
    );

    let second = backend
        .allocate_resident(16)
        .expect("Fix: second CUDA resident buffer allocation failed.");
    assert_eq!(
        backend.resident_allocated_bytes(),
        24,
        "Fix: resident byte accounting must be cumulative across handles."
    );

    backend
        .free_resident(first)
        .expect("Fix: CUDA first resident free failed.");
    assert_eq!(
        backend.resident_allocated_bytes(),
        16,
        "Fix: freeing one resident handle must subtract only that handle's bytes."
    );

    backend
        .free_resident(second)
        .expect("Fix: CUDA second resident free failed.");
    assert_eq!(
        backend.resident_allocated_bytes(),
        0,
        "Fix: freeing every resident handle must return accounting to zero."
    );
}

#[test]
fn resident_buffer_download_into_preserves_caller_storage() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");
    let mut output = Vec::with_capacity(64);
    let output_ptr = output.as_ptr() as usize;
    backend
        .download_resident_into(handle, &mut output)
        .expect("Fix: CUDA resident download_into failed.");

    assert_eq!(output, input);
    assert_eq!(output.as_ptr() as usize, output_ptr);
    assert!(output.capacity() >= 64);

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_range_download_into_preserves_caller_storage() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![10u8, 11, 12, 13, 14, 15, 16, 17];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");
    let mut output = Vec::with_capacity(32);
    let output_ptr = output.as_ptr() as usize;
    backend
        .download_resident_range_into(handle, 2, 4, &mut output)
        .expect("Fix: CUDA resident range download_into failed.");

    assert_eq!(output, vec![12, 13, 14, 15]);
    assert_eq!(output.as_ptr() as usize, output_ptr);
    assert!(output.capacity() >= 32);

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_adjacent_partial_uploads_fuse_same_handle_h2d_copy() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let initial = vec![0u8; 8];
    let first_patch = [1u8, 2, 3];
    let second_patch = [4u8, 5, 6];
    let handle = backend
        .allocate_resident(initial.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &initial)
        .expect("Fix: initial CUDA resident upload failed.");

    backend.reset_telemetry();
    backend
        .upload_resident_at_many(&[
            (handle, 0, first_patch.as_slice()),
            (handle, 3, second_patch.as_slice()),
        ])
        .expect("Fix: adjacent CUDA resident partial upload failed.");
    let output = backend
        .download_resident(handle)
        .expect("Fix: CUDA resident download after adjacent partial upload failed.");

    assert_eq!(output, vec![1, 2, 3, 4, 5, 6, 0, 0]);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 2,
        "Fix: adjacent partial upload plus verification download should use one upload fence and one download fence."
    );
    assert_eq!(
        telemetry.host_upload_operations, 1,
        "Fix: adjacent same-handle partial uploads must fuse to one H2D copy."
    );
    assert_eq!(
        telemetry.host_to_device_bytes, 6,
        "Fix: adjacent partial upload fusion must account the fused byte interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]

fn resident_buffer_overlapping_partial_uploads_preserve_later_write_and_fuse_bytes() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let initial = vec![0u8; 8];
    let first_patch = [1u8, 2, 3, 4];
    let second_patch = [9u8, 8];
    let handle = backend
        .allocate_resident(initial.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &initial)
        .expect("Fix: initial CUDA resident upload failed.");

    backend.reset_telemetry();
    backend
        .upload_resident_at_many(&[
            (handle, 1, first_patch.as_slice()),
            (handle, 3, second_patch.as_slice()),
        ])
        .expect("Fix: overlapping CUDA resident partial upload failed.");
    let output = backend
        .download_resident(handle)
        .expect("Fix: CUDA resident download after overlapping partial upload failed.");

    assert_eq!(output, vec![0, 1, 2, 9, 8, 0, 0, 0]);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.host_upload_operations, 1,
        "Fix: overlapping same-handle partial uploads must fuse to one H2D copy."
    );
    assert_eq!(
        telemetry.host_to_device_bytes, 4,
        "Fix: overlapping partial upload fusion must account only the final fused byte interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_backward_overlapping_partial_uploads_fuse_and_preserve_order() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let initial = vec![0u8; 10];
    let first_patch = [4u8, 5, 6, 7];
    let second_patch = [1u8, 2, 9, 8];
    let handle = backend
        .allocate_resident(initial.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &initial)
        .expect("Fix: initial CUDA resident upload failed.");

    backend.reset_telemetry();
    backend
        .upload_resident_at_many(&[
            (handle, 4, first_patch.as_slice()),
            (handle, 2, second_patch.as_slice()),
        ])
        .expect("Fix: backward-overlapping CUDA resident partial upload failed.");
    let output = backend
        .download_resident(handle)
        .expect("Fix: CUDA resident download after backward-overlap partial upload failed.");

    assert_eq!(output, vec![0, 0, 1, 2, 9, 8, 6, 7, 0, 0]);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.host_upload_operations, 1,
        "Fix: backward-overlapping same-handle partial uploads must fuse to one H2D copy."
    );
    assert_eq!(
        telemetry.host_to_device_bytes, 6,
        "Fix: backward-overlapping partial upload fusion must account only the final fused byte interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_rejects_wrong_upload_size() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let handle = backend
        .allocate_resident(8)
        .expect("Fix: CUDA resident buffer allocation failed.");
    let err = backend
        .upload_resident(handle, &[1, 2, 3])
        .expect_err("Fix: wrong-sized resident upload must fail.");
    assert!(
        err.to_string().contains("expected 8 bytes"),
        "Fix: resident upload size errors must state the expected byte length, got: {err}"
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn partial_resident_upload_releases_stream_on_copy_errors() {
    let source = include_str!("../src/backend/resident_io.rs");
    let partial_upload = source
        .split("pub fn upload_resident_at_many")
        .nth(1)
        .and_then(|tail| tail.split("pub fn resident_device_ptr").next())
        .expect(
            "Fix: resident_io.rs must expose upload_resident_at_many before resident_device_ptr.",
        );

    assert!(
        source.contains("fn with_resident_stream_classified")
            && partial_upload.contains("self.with_resident_stream_classified(|stream|")
            && partial_upload.contains("ResidentStreamFailure::CompletionUnproven")
            && partial_upload.contains("std::mem::forget(host_transfers)"),
        "Fix: partial CUDA resident uploads must route through the resident pooled-stream helper and retain host staging when stream completion is unproven."
    );
}

#[test]
fn resident_inflight_reference_counting_does_not_saturate_underflow() {
    let source = include_str!("../src/backend/resident.rs");
    assert!(
        source.contains("checked_atomic_sub_usize_with_order")
            && !source.contains(concat!(".", "saturating_sub")),
        "Fix: CUDA resident in-flight reference counting must fail loudly on underflow instead of hiding lifetime bugs."
    );
}
