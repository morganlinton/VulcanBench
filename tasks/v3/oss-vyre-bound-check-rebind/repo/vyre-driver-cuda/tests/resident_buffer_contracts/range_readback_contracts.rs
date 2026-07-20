use vyre_driver_cuda::CudaBackend;

#[test]
fn resident_buffer_single_range_download_uses_one_batched_readback_path() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![40u8, 41, 42, 43, 44, 45, 46, 47];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");

    let mut output = Vec::with_capacity(32);
    let output_ptr = output.as_ptr() as usize;
    backend.reset_telemetry();
    backend
        .download_resident_range_into(handle, 1, 5, &mut output)
        .expect("Fix: CUDA resident single range download_into failed.");

    assert_eq!(output.as_slice(), &input[1..6]);
    assert_eq!(output.as_ptr() as usize, output_ptr);
    assert!(output.capacity() >= 32);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: single resident range download must use the shared one-fence batched readback path."
    );
    assert_eq!(
        telemetry.device_readback_operations, 1,
        "Fix: single resident range download must report exactly one D2H copy."
    );
    assert_eq!(
        telemetry.readback_bytes, 5,
        "Fix: single resident range download must account the requested byte interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_batched_range_downloads_share_one_sync_point() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let first_input = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
    let second_input = vec![10u8, 11, 12, 13, 14, 15, 16, 17];
    let first = backend
        .allocate_resident(first_input.len())
        .expect("Fix: CUDA first resident buffer allocation failed.");
    let second = backend
        .allocate_resident(second_input.len())
        .expect("Fix: CUDA second resident buffer allocation failed.");

    backend
        .upload_resident(first, &first_input)
        .expect("Fix: CUDA first resident upload failed.");
    backend
        .upload_resident(second, &second_input)
        .expect("Fix: CUDA second resident upload failed.");

    backend.reset_telemetry();
    let mut first_out = Vec::with_capacity(32);
    let mut second_out = Vec::with_capacity(32);
    let first_ptr = first_out.as_ptr() as usize;
    let second_ptr = second_out.as_ptr() as usize;
    backend
        .download_resident_ranges_into(
            &[(first, 2, 3), (second, 4, 2)],
            &mut [&mut first_out, &mut second_out],
        )
        .expect("Fix: CUDA batched resident range download failed.");

    assert_eq!(first_out, vec![2, 3, 4]);
    assert_eq!(second_out, vec![14, 15]);
    assert_eq!(first_out.as_ptr() as usize, first_ptr);
    assert_eq!(second_out.as_ptr() as usize, second_ptr);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: batched resident range download must use one CUDA stream fence, not one sync per range."
    );
    assert_eq!(
        telemetry.device_readback_operations, 2,
        "Fix: batched resident range download must still count both D2H copies."
    );
    assert_eq!(
        telemetry.readback_bytes, 5,
        "Fix: batched resident range download must account only requested range bytes."
    );

    backend
        .free_resident(first)
        .expect("Fix: CUDA first resident free failed.");
    backend
        .free_resident(second)
        .expect("Fix: CUDA second resident free failed.");
}

#[test]
fn resident_buffer_batched_range_downloads_fuse_same_handle_overlaps() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input: Vec<u8> = (0..16).map(|value| value as u8).collect();
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");

    backend.reset_telemetry();
    let mut first_out = Vec::with_capacity(32);
    let mut second_out = Vec::with_capacity(32);
    let mut third_out = Vec::with_capacity(32);
    let first_ptr = first_out.as_ptr() as usize;
    let second_ptr = second_out.as_ptr() as usize;
    let third_ptr = third_out.as_ptr() as usize;
    backend
        .download_resident_ranges_into(
            &[(handle, 0, 8), (handle, 4, 8), (handle, 12, 4)],
            &mut [&mut first_out, &mut second_out, &mut third_out],
        )
        .expect("Fix: overlapping same-handle resident range download failed.");

    assert_eq!(first_out.as_slice(), &input[0..8]);
    assert_eq!(second_out.as_slice(), &input[4..12]);
    assert_eq!(third_out.as_slice(), &input[12..16]);
    assert_eq!(first_out.as_ptr() as usize, first_ptr);
    assert_eq!(second_out.as_ptr() as usize, second_ptr);
    assert_eq!(third_out.as_ptr() as usize, third_ptr);
    assert!(first_out.capacity() >= 32);
    assert!(second_out.capacity() >= 32);
    assert!(third_out.capacity() >= 32);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: fused same-handle resident range download must use one CUDA stream fence."
    );
    assert_eq!(
        telemetry.device_readback_operations, 1,
        "Fix: overlapping same-handle resident ranges must fuse to one D2H copy."
    );
    assert_eq!(
        telemetry.readback_bytes, 16,
        "Fix: overlapping same-handle resident ranges must account the fused D2H byte interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_batched_range_downloads_clear_zero_slots_while_fusing_nonzero() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input: Vec<u8> = (20..36).map(|value| value as u8).collect();
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");

    backend.reset_telemetry();
    let mut empty_out = Vec::with_capacity(32);
    let mut first_out = Vec::with_capacity(32);
    let mut second_out = Vec::with_capacity(32);
    empty_out.extend_from_slice(&[99, 99]);
    first_out.extend_from_slice(&[88, 88]);
    second_out.extend_from_slice(&[77, 77]);
    let empty_ptr = empty_out.as_ptr() as usize;
    let first_ptr = first_out.as_ptr() as usize;
    let second_ptr = second_out.as_ptr() as usize;
    backend
        .download_resident_ranges_into(
            &[(handle, 0, 0), (handle, 2, 4), (handle, 3, 2)],
            &mut [&mut empty_out, &mut first_out, &mut second_out],
        )
        .expect("Fix: mixed zero/nonzero resident range download failed.");

    assert!(empty_out.is_empty());
    assert_eq!(first_out.as_slice(), &input[2..6]);
    assert_eq!(second_out.as_slice(), &input[3..5]);
    assert_eq!(empty_out.as_ptr() as usize, empty_ptr);
    assert_eq!(first_out.as_ptr() as usize, first_ptr);
    assert_eq!(second_out.as_ptr() as usize, second_ptr);
    assert!(empty_out.capacity() >= 32);
    assert!(first_out.capacity() >= 32);
    assert!(second_out.capacity() >= 32);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: mixed zero/nonzero fused resident range download must use one CUDA stream fence."
    );
    assert_eq!(
        telemetry.device_readback_operations, 1,
        "Fix: zero-byte slots must not force extra D2H copies when nonzero ranges fuse."
    );
    assert_eq!(
        telemetry.readback_bytes, 4,
        "Fix: mixed zero/nonzero resident ranges must account only the fused nonzero byte interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn resident_buffer_batched_full_download_into_reuses_output_slots() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let first_input = vec![1u8, 2, 3, 4];
    let second_input = vec![10u8, 11, 12, 13, 14];
    let first = backend
        .allocate_resident(first_input.len())
        .expect("Fix: CUDA first resident buffer allocation failed.");
    let second = backend
        .allocate_resident(second_input.len())
        .expect("Fix: CUDA second resident buffer allocation failed.");

    backend
        .upload_resident(first, &first_input)
        .expect("Fix: CUDA first resident upload failed.");
    backend
        .upload_resident(second, &second_input)
        .expect("Fix: CUDA second resident upload failed.");

    let mut outputs = vec![Vec::with_capacity(32), Vec::with_capacity(32)];
    let first_ptr = outputs[0].as_ptr() as usize;
    let second_ptr = outputs[1].as_ptr() as usize;
    backend.reset_telemetry();
    backend
        .download_resident_many_into(&[first, second], &mut outputs)
        .expect("Fix: CUDA batched resident full download_into failed.");

    assert_eq!(outputs[0], first_input);
    assert_eq!(outputs[1], second_input);
    assert_eq!(outputs[0].as_ptr() as usize, first_ptr);
    assert_eq!(outputs[1].as_ptr() as usize, second_ptr);
    assert!(outputs[0].capacity() >= 32);
    assert!(outputs[1].capacity() >= 32);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: batched full resident download_into must use one CUDA stream fence."
    );
    assert_eq!(
        telemetry.device_readback_operations, 2,
        "Fix: batched full resident download_into must count both D2H copies."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (first_input.len() + second_input.len()) as u64,
        "Fix: batched full resident download_into must account full resident bytes."
    );

    backend
        .free_resident(first)
        .expect("Fix: CUDA first resident free failed.");
    backend
        .free_resident(second)
        .expect("Fix: CUDA second resident free failed.");
}

#[test]
fn resident_buffer_duplicate_full_downloads_fuse_same_handle_readback() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![90u8, 91, 92, 93, 94, 95, 96, 97];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");

    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");

    let mut outputs = vec![Vec::with_capacity(32), Vec::with_capacity(32)];
    let first_ptr = outputs[0].as_ptr() as usize;
    let second_ptr = outputs[1].as_ptr() as usize;
    backend.reset_telemetry();
    backend
        .download_resident_many_into(&[handle, handle], &mut outputs)
        .expect("Fix: duplicate CUDA resident full download_into failed.");

    assert_eq!(outputs[0], input);
    assert_eq!(outputs[1], input);
    assert_eq!(outputs[0].as_ptr() as usize, first_ptr);
    assert_eq!(outputs[1].as_ptr() as usize, second_ptr);
    assert!(outputs[0].capacity() >= 32);
    assert!(outputs[1].capacity() >= 32);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: duplicate full resident download must use one CUDA stream fence."
    );
    assert_eq!(
        telemetry.device_readback_operations, 1,
        "Fix: duplicate full resident downloads from the same handle must fuse to one D2H copy."
    );
    assert_eq!(
        telemetry.readback_bytes,
        input.len() as u64,
        "Fix: duplicate full resident download must account only the fused device interval."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn zero_byte_resident_range_batch_download_does_not_sync_or_readback() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");
    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");

    let mut output = Vec::with_capacity(32);
    output.extend_from_slice(&[9, 9, 9]);
    let output_ptr = output.as_ptr() as usize;
    backend.reset_telemetry();
    backend
        .download_resident_ranges_into(&[(handle, 4, 0)], &mut [&mut output])
        .expect("Fix: zero-byte CUDA resident range batch download must succeed.");

    assert!(output.is_empty());
    assert_eq!(output.as_ptr() as usize, output_ptr);
    assert!(output.capacity() >= 32);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 0,
        "Fix: zero-byte resident batch readback must not acquire/synchronize a CUDA stream."
    );
    assert_eq!(
        telemetry.device_readback_operations, 0,
        "Fix: zero-byte resident batch readback must not report D2H operations."
    );
    assert_eq!(
        telemetry.readback_bytes, 0,
        "Fix: zero-byte resident batch readback must not report bytes."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}

#[test]
fn invalid_resident_range_batch_download_preserves_caller_outputs() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let input = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let handle = backend
        .allocate_resident(input.len())
        .expect("Fix: CUDA resident buffer allocation failed.");
    backend
        .upload_resident(handle, &input)
        .expect("Fix: CUDA resident upload failed.");

    let mut output = Vec::with_capacity(32);
    output.extend_from_slice(&[9, 9, 9]);
    let output_ptr = output.as_ptr() as usize;
    backend.reset_telemetry();
    let err = backend
        .download_resident_ranges_into(&[(handle, 6, 4)], &mut [&mut output])
        .expect_err("Fix: out-of-bounds resident batch range must fail.");

    assert!(
        err.to_string().contains("requested bytes [6..10)"),
        "Fix: resident range validation error must include the offending byte interval, got: {err}"
    );
    assert_eq!(
        output,
        vec![9, 9, 9],
        "Fix: invalid resident range validation must not clear or partially rewrite caller output."
    );
    assert_eq!(output.as_ptr() as usize, output_ptr);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 0,
        "Fix: invalid resident range validation must fail before acquiring/synchronizing a CUDA stream."
    );
    assert_eq!(
        telemetry.readback_bytes, 0,
        "Fix: invalid resident range validation must fail before D2H accounting."
    );

    backend
        .free_resident(handle)
        .expect("Fix: CUDA resident free failed.");
}
