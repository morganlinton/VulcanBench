use super::*;

#[test]
fn wgpu_backend_allocates_uploads_batches_and_frees_resident_buffers() {
    let backend = backend();
    let first = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate resident buffers");
    let second = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate a second resident buffer");

    backend
        .upload_resident_many(&[(&first, &[1, 2, 3, 4]), (&second, &[5, 6, 7, 8])])
        .expect(
            "WGPU backend must batch resident uploads without falling back to unsupported defaults",
        );

    let mut first_readback = Vec::with_capacity(64);
    backend
        .download_resident_range_into(&first, 1, 3, &mut first_readback)
        .expect("WGPU backend must ranged-download resident buffers into caller-owned scratch");
    assert_eq!(first_readback, vec![2, 3, 4]);
    assert!(
        first_readback.capacity() >= 64,
        "resident ranged download must preserve caller scratch capacity"
    );

    let second_readback = backend
        .download_resident(&second)
        .expect("WGPU backend must download complete resident buffers");
    assert_eq!(
        &second_readback[..8],
        &[5, 6, 7, 8, 0, 0, 0, 0],
        "full resident readback must return uploaded prefix and padded zero fill"
    );

    backend
        .free_resident(first)
        .expect("WGPU backend must free first resident buffer");
    backend
        .free_resident(second)
        .expect("WGPU backend must free second resident buffer");
}

#[test]
fn wgpu_backend_ranged_upload_updates_only_requested_resident_bytes() {
    let backend = backend();
    let resource = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate resident buffers");
    backend
        .upload_resident(&resource, &[0x10; 16])
        .expect("initial full resident upload must succeed");

    backend
        .upload_resident_at(&resource, 4, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("WGPU backend must support aligned ranged resident uploads");

    let bytes = backend
        .download_resident(&resource)
        .expect("resident buffer must download after ranged upload");
    assert_eq!(
        &bytes[..16],
        &[0x10, 0x10, 0x10, 0x10, 1, 2, 3, 4, 5, 6, 7, 8, 0x10, 0x10, 0x10, 0x10],
        "ranged resident upload must mutate only the requested byte range"
    );

    backend
        .free_resident(resource)
        .expect("ranged-upload resident buffer must free cleanly");
}

#[test]
fn wgpu_backend_invalid_single_range_download_preserves_caller_output() {
    let backend = backend();
    let resource = backend
        .allocate_resident(4)
        .expect("WGPU backend must allocate resident buffer");
    backend
        .upload_resident(&resource, &[1, 2, 3, 4])
        .expect("initial resident upload must succeed");

    let mut output = Vec::with_capacity(32);
    output.extend_from_slice(&[9, 9, 9]);
    let capacity = output.capacity();
    let error = backend
        .download_resident_range_into(&resource, 3, 2, &mut output)
        .expect_err("invalid single resident range download must reject before readback");

    assert!(
        error.to_string().contains("byte range [3..5)"),
        "invalid single range error must describe the requested byte range, got: {error}"
    );
    assert_eq!(
        output,
        vec![9, 9, 9],
        "invalid single range download must not clobber caller-owned output"
    );
    assert_eq!(output.capacity(), capacity);

    backend
        .free_resident(resource)
        .expect("resident buffer must free cleanly after rejected range download");
}


