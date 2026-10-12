//! WGPU pipeline module-boundary and persistent-output contracts.

use std::fs;
use std::path::PathBuf;

#[test]
fn parent_pipeline_module_orchestrates_instead_of_owning_every_duty() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pipeline = crate_root.join("src/pipeline.rs");
    let contents = fs::read_to_string(&pipeline)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", pipeline.display()));

    let forbidden = [
        "impl CompiledPipeline for WgpuPipeline",
        "pub(crate) struct BufferBindingInfo",
        "fn descriptor_buffer_bindings",
        "fn bind_group_layout_fingerprint",
        "fn create_bind_group_layouts",
    ];
    let mut findings = Vec::new();
    for pattern in forbidden {
        if contents.contains(pattern) {
            findings.push(pattern);
        }
    }

    let line_count = contents.lines().count();
    assert!(
        line_count <= 900,
        "src/pipeline.rs has {line_count} lines. Fix: keep orchestration in the parent and move execution, metadata, caching, and validation duties into one-purpose modules."
    );
    assert!(
        findings.is_empty(),
        "src/pipeline.rs reabsorbed non-orchestration duties: {}. Fix: keep descriptor metadata in pipeline/descriptor_metadata.rs and dispatch trait entrypoints in pipeline/compiled_dispatch.rs.",
        findings.join(", ")
    );
}

#[test]
fn persistent_output_paths_use_trimmed_prefix_readback() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scan_files = [
        crate_root.join("src/pipeline.rs"),
        crate_root.join("src/pipeline/compiled_dispatch.rs"),
    ];
    let mut findings = Vec::new();

    for path in scan_files {
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        if contents.contains("readback_until(") {
            findings.push(
                path.strip_prefix(&crate_root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }

    assert!(
        findings.is_empty(),
        "persistent pipeline output paths must not full-readback GPU allocations: {}. Fix: use pipeline/output_readback.rs so output_byte_range transfers only meaningful bytes.",
        findings.join(", ")
    );
}

#[test]
fn persistent_resource_output_dispatch_does_not_read_back_outputs() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dispatch = crate_root.join("src/pipeline/compiled_dispatch.rs");
    let contents = fs::read_to_string(&dispatch)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dispatch.display()));
    let body = contents
        .split("fn dispatch_persistent_resource_outputs(")
        .nth(1)
        .and_then(|tail| tail.split("fn dispatch_persistent_handles_batched").next())
        .expect("dispatch_persistent_resource_outputs body must remain discoverable");

    assert!(
        body.contains("resolve_persistent_resources_for_resource_outputs"),
        "persistent resource-output dispatch must validate resident output handles through the single-pass resolver before launch"
    );
    assert!(
        !body.contains("readback_persistent_outputs"),
        "persistent resource-output dispatch must not read output buffers back to the host"
    );
    assert!(
        body.contains("dispatch_borrowed_persistent_batched"),
        "persistent resource-output dispatch must still execute the compiled GPU pipeline"
    );
}
