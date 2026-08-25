use super::*;

#[test]
fn conformance_tests_use_wrapped_backend_acquisition() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let tests_dir = manifest_dir.join("tests");
    let mut findings = Vec::new();
    scan_for_raw_backend_factory_calls(&src_dir, &src_dir, &mut findings);
    scan_for_raw_backend_factory_calls(&tests_dir, &tests_dir, &mut findings);

    assert!(
        findings.is_empty(),
        "Fix: conformance runner code must call BackendRegistration::acquire() so grid-sync split and backend wrappers are applied:\n{}",
        findings.join("\n")
    );
}

#[test]
fn dispatch_conformance_isolates_backend_instance_per_pair() {
    let source = repo_file("conform/vyre-conform-runner/src/main.rs");
    let dispatch_start = source
        .find("fn dispatch_pairs(")
        .expect("Fix: conformance runner must expose dispatch_pairs.");
    let dispatch_end = source[dispatch_start..]
        .find("fn acquire_backend(")
        .map(|offset| dispatch_start + offset)
        .expect("Fix: dispatch_pairs must remain before acquire_backend.");
    let dispatch = &source[dispatch_start..dispatch_end];
    let prepare_pos = dispatch
        .find("let prepared = match prepare_entry(entry)")
        .expect("Fix: dispatch_pairs must prepare each entry before backend comparison.");
    let acquire_pos = dispatch
        .find("let backend = match acquire_backend(&backend_id)")
        .expect("Fix: dispatch_pairs must acquire the selected backend.");
    let compare_pos = dispatch
        .find("compare_backend_against_reference(")
        .expect("Fix: dispatch_pairs must compare backend output against reference.");
    assert!(
        prepare_pos < acquire_pos && acquire_pos < compare_pos,
        "Fix: dispatch conformance must acquire a fresh backend per prepared pair so a poisoned CUDA instance cannot taint later release evidence."
    );
    assert!(
        !dispatch[..prepare_pos].contains("acquire_backend(&backend_id)"),
        "Fix: dispatch conformance must not share one backend instance across all selected pairs."
    );
}

#[test]
fn release_conformance_static_sizing_uses_packed_buffer_lengths() {
    for path in [
        "conform/vyre-conform-runner/src/witness_plan.rs",
        "conform/vyre-conform-runner/tests/__split/parity_matrix_chunk1.rs",
        "vyre-libs/src/primitive_catalog.rs",
    ] {
        let source = repo_file(path);
        assert!(
            source.contains(".static_byte_len()"),
            "Fix: `{path}` must use BufferDecl::static_byte_len() so sub-byte static buffers use packed lengths."
        );
        assert!(
            !source.contains("buffer.element().min_bytes()"),
            "Fix: `{path}` must not size static dispatch buffers with min_bytes(); I4/FP4/NF4 buffers require packed byte lengths."
        );
    }

    for path in [
        "vyre-reference/src/execution/hashmap/mod.rs",
        "vyre-reference/src/execution/hashmap/memory.rs",
    ] {
        let source = repo_file(path);
        assert!(
            source.contains(".static_byte_len()"),
            "Fix: `{path}` must use BufferDecl::static_byte_len() so reference allocation mirrors packed backend buffer lengths."
        );
    }

    for path in [
        "vyre-reference/src/execution/hashmap/sync.rs",
        "vyre-reference/src/oob.rs",
    ] {
        let source = repo_file(path);
        assert!(
            source.contains(".bit_width()"),
            "Fix: `{path}` must compute logical element counts from DataType::bit_width() so sub-byte buffers report packed logical lengths."
        );
    }
}

