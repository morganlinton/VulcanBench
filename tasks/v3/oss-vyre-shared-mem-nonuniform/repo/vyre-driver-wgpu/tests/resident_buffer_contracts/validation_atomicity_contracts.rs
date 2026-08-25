use super::*;

#[test]
fn wgpu_backend_rejects_stale_and_borrowed_resident_handles() {
    let backend = backend();
    let resident = backend
        .allocate_resident(4)
        .expect("WGPU backend must allocate resident buffers");
    let stale = resident.clone();
    backend
        .free_resident(resident)
        .expect("first resident free must succeed");
    let err = backend
        .upload_resident(&stale, &[1, 2, 3, 4])
        .expect_err("stale resident upload must fail loudly");
    assert!(
        err.to_string().contains("stale handle"),
        "stale upload error must explain stale resident handles, got: {err}"
    );

    let borrowed = Resource::Borrowed(vec![0; 4]);
    let err = backend
        .free_resident(borrowed)
        .expect_err("borrowed resource free must fail loudly");
    assert!(
        err.to_string().contains("borrowed resource"),
        "borrowed free error must explain resource kind, got: {err}"
    );

    let borrowed = Resource::Borrowed(vec![0; 4]);
    let err = backend
        .upload_resident_at(&borrowed, 0, &[1, 2, 3, 4])
        .expect_err("borrowed ranged upload must fail loudly");
    assert!(
        err.to_string().contains("borrowed resource"),
        "borrowed ranged upload error must explain resource kind, got: {err}"
    );

    let err = backend
        .upload_resident_at(&stale, 0, &[1, 2, 3, 4])
        .expect_err("stale ranged upload must fail loudly");
    assert!(
        err.to_string().contains("stale handle"),
        "stale ranged upload error must explain stale resident handles, got: {err}"
    );
}

#[test]
fn wgpu_backend_batch_upload_validates_before_any_write() {
    let backend = backend();
    let first = backend
        .allocate_resident(4)
        .expect("WGPU backend must allocate first resident buffer");
    let second = backend
        .allocate_resident(4)
        .expect("WGPU backend must allocate second resident buffer");
    backend
        .upload_resident_many(&[(&first, &[9]), (&second, &[8])])
        .expect("initial resident uploads must succeed");

    let oversized = [0u8; 8];
    let err = backend
        .upload_resident_many(&[(&first, &[1, 2, 3, 4]), (&second, &oversized)])
        .expect_err("invalid second upload must reject the entire batch");
    assert!(
        err.to_string().contains("batch upload"),
        "batch upload error must name the failing operation, got: {err}"
    );

    let mut first_readback = Vec::new();
    backend
        .download_resident_range_into(&first, 0, 4, &mut first_readback)
        .expect("first resident readback must succeed after rejected batch");
    assert_eq!(
        first_readback,
        vec![9, 0, 0, 0],
        "batch upload must not partially update earlier resources when a later upload is invalid"
    );

    backend
        .free_resident(first)
        .expect("first resident buffer must free cleanly");
    backend
        .free_resident(second)
        .expect("second resident buffer must free cleanly");
}

#[test]
fn wgpu_backend_ranged_batch_upload_validates_before_any_write() {
    let backend = backend();
    let first = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate first resident buffer");
    let second = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate second resident buffer");
    backend
        .upload_resident_many(&[(&first, &[9; 16]), (&second, &[8; 16])])
        .expect("initial resident uploads must succeed");

    let err = backend
        .upload_resident_at_many(&[
            (&first, 4, &[1, 2, 3, 4]),
            (&second, 12, &[5, 6, 7, 8, 9, 10, 11, 12]),
        ])
        .expect_err("invalid second ranged upload must reject the entire batch");
    assert!(
        err.to_string().contains("ranged batch upload"),
        "ranged batch upload error must name the failing operation, got: {err}"
    );

    let mut first_readback = Vec::new();
    backend
        .download_resident_range_into(&first, 0, 16, &mut first_readback)
        .expect("first resident readback must succeed after rejected ranged batch");
    assert_eq!(
        first_readback,
        vec![9; 16],
        "ranged batch upload must not partially update earlier resources when a later range is invalid"
    );

    backend
        .free_resident(first)
        .expect("first resident buffer must free cleanly");
    backend
        .free_resident(second)
        .expect("second resident buffer must free cleanly");
}

#[test]
fn wgpu_backend_ranged_batch_alignment_error_writes_nothing() {
    let backend = backend();
    let first = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate first resident buffer");
    let second = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate second resident buffer");
    backend
        .upload_resident_many(&[(&first, &[3; 16]), (&second, &[4; 16])])
        .expect("initial resident uploads must succeed");

    let err = backend
        .upload_resident_at_many(&[(&first, 4, &[9, 9, 9, 9]), (&second, 2, &[1, 2, 3, 4])])
        .expect_err("unaligned ranged upload must reject the entire batch");
    assert!(
        err.to_string().contains("aligned"),
        "alignment failure must explain the WGPU copy alignment contract, got: {err}"
    );

    let first_bytes = backend
        .download_resident(&first)
        .expect("first resident buffer must download after rejected alignment batch");
    let second_bytes = backend
        .download_resident(&second)
        .expect("second resident buffer must download after rejected alignment batch");
    assert_eq!(
        &first_bytes[..16],
        &[3; 16],
        "alignment rejection must not partially update the already-valid first range"
    );
    assert_eq!(
        &second_bytes[..16],
        &[4; 16],
        "alignment rejection must not update the invalid second range"
    );

    backend
        .free_resident(first)
        .expect("first resident buffer must free cleanly");
    backend
        .free_resident(second)
        .expect("second resident buffer must free cleanly");
}

#[test]

fn wgpu_backend_ranged_batch_download_validates_before_any_readback() {
    let backend = backend();
    let first = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate first resident buffer");
    let second = backend
        .allocate_resident(16)
        .expect("WGPU backend must allocate second resident buffer");
    backend
        .upload_resident_many(&[(&first, &[1; 16]), (&second, &[2; 16])])
        .expect("initial resident uploads must succeed");

    let mut first_out = vec![0xaa];
    let mut second_out = vec![0xbb];
    let err = backend
        .download_resident_ranges_into(
            &[(&first, 0, 4), (&second, 12, 8)],
            &mut [&mut first_out, &mut second_out],
        )
        .expect_err("invalid second ranged download must reject the entire batch");
    assert!(
        err.to_string().contains("ranged batch download"),
        "ranged batch download error must name the failing operation, got: {err}"
    );
    assert_eq!(
        first_out,
        vec![0xaa],
        "ranged batch download must not mutate an earlier output before a later range fails validation"
    );
    assert_eq!(
        second_out,
        vec![0xbb],
        "ranged batch download must not mutate the invalid output"
    );

    backend
        .free_resident(first)
        .expect("first resident buffer must free cleanly");
    backend
        .free_resident(second)
        .expect("second resident buffer must free cleanly");
}

