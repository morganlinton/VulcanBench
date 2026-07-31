use super::*;

#[test]
fn wgpu_backend_ranged_batch_upload_updates_multiple_resources() {
    let backend = backend();
    let first = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate first resident buffer");
    let second = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate second resident buffer");
    backend
        .upload_resident_many(&[(&first, &[0x10; 16]), (&second, &[0x20; 16])])
        .expect("initial resident uploads must succeed");

    backend
        .upload_resident_at_many(&[(&first, 4, &[1, 2, 3, 4]), (&second, 8, &[5, 6, 7, 8])])
        .expect("WGPU backend must support successful ranged batch resident uploads");

    let first_bytes = backend
        .download_resident(&first)
        .expect("first resident buffer must download after ranged batch upload");
    let second_bytes = backend
        .download_resident(&second)
        .expect("second resident buffer must download after ranged batch upload");
    assert_eq!(
        &first_bytes[..16],
        &[0x10, 0x10, 0x10, 0x10, 1, 2, 3, 4, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10],
        "ranged batch upload must update only the first resource range"
    );
    assert_eq!(
        &second_bytes[..16],
        &[0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 5, 6, 7, 8, 0x20, 0x20, 0x20, 0x20],
        "ranged batch upload must update only the second resource range"
    );

    backend
        .free_resident(first)
        .expect("first resident buffer must free cleanly");
    backend
        .free_resident(second)
        .expect("second resident buffer must free cleanly");
}

#[test]
fn wgpu_backend_ranged_batch_download_reads_multiple_resources() {
    let backend = backend();
    let first = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate first resident buffer");
    let second = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate second resident buffer");
    backend
        .upload_resident_many(&[
            (&first, &[0, 1, 2, 3, 4, 5, 6, 7]),
            (&second, &[8, 9, 10, 11, 12, 13, 14, 15]),
        ])
        .expect("initial resident uploads must succeed");

    let mut first_out = Vec::with_capacity(64);
    let mut second_out = Vec::with_capacity(64);
    backend
        .download_resident_ranges_into(
            &[(&first, 2, 4), (&second, 4, 4)],
            &mut [&mut first_out, &mut second_out],
        )
        .expect("WGPU backend must support ranged batch resident downloads");
    assert_eq!(first_out, vec![2, 3, 4, 5]);
    assert_eq!(second_out, vec![12, 13, 14, 15]);
    assert!(
        first_out.capacity() >= 64 && second_out.capacity() >= 64,
        "resident ranged batch download must preserve caller scratch capacity"
    );

    backend
        .free_resident(first)
        .expect("first resident buffer must free cleanly");
    backend
        .free_resident(second)
        .expect("second resident buffer must free cleanly");
}

#[test]
fn wgpu_backend_ranged_batch_download_fuses_overlapping_same_handle_ranges() {
    let backend = backend();
    let resident = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate resident buffer for fused range readback");
    backend
        .upload_resident(&resident, &[0, 1, 2, 3, 4, 5, 6, 7])
        .expect("initial resident upload must succeed");

    let mut first_out = Vec::with_capacity(64);
    let mut second_out = Vec::with_capacity(64);
    let mut empty_out = vec![0xaa];
    backend
        .download_resident_ranges_into(
            &[(&resident, 0, 4), (&resident, 2, 4), (&resident, 6, 0)],
            &mut [&mut first_out, &mut second_out, &mut empty_out],
        )
        .expect("WGPU backend must fuse overlapping resident range readbacks without changing caller outputs");

    assert_eq!(first_out, vec![0, 1, 2, 3]);
    assert_eq!(second_out, vec![2, 3, 4, 5]);
    assert!(
        empty_out.is_empty(),
        "zero-byte fused resident views must clear the caller output slot"
    );
    assert!(
        first_out.capacity() >= 64 && second_out.capacity() >= 64,
        "fused resident ranged batch download must preserve caller scratch capacity"
    );

    backend
        .free_resident(resident)
        .expect("fused resident readback buffer must free cleanly");
}

