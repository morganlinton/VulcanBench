//! Batch megakernel sparse-hit writer selection contracts.

#![cfg(feature = "megakernel-batch")]

use vyre_driver_wgpu::megakernel::BatchHitWriter;

#[test]
fn auto_hit_writer_selects_hierarchical_when_subgroups_exist() {
    assert_eq!(
        BatchHitWriter::Auto
            .resolve_for_backend(true)
            .expect("auto selection must resolve on subgroup-capable backends"),
        BatchHitWriter::HierarchicalSubgroup
    );
}

#[test]
fn auto_hit_writer_keeps_scalar_on_non_subgroup_backend() {
    assert_eq!(
        BatchHitWriter::Auto
            .resolve_for_backend(false)
            .expect("auto selection must still resolve without subgroups"),
        BatchHitWriter::Scalar
    );
}

#[test]
fn explicit_hierarchical_hit_writer_fails_without_subgroups() {
    let error = BatchHitWriter::HierarchicalSubgroup
        .resolve_for_backend(false)
        .expect_err("explicit subgroup atomics must not silently downgrade");

    let rendered = error.to_string();
    assert!(
        rendered.contains("supports_subgroup_ops=false"),
        "error should name the missing subgroup capability: {rendered}"
    );
    assert!(
        !rendered.to_ascii_lowercase().contains("cpu fallback"),
        "missing subgroup support must not be framed as a CPU fallback: {rendered}"
    );
}

#[test]
fn explicit_scalar_hit_writer_remains_scalar_on_subgroup_backend() {
    assert_eq!(
        BatchHitWriter::Scalar
            .resolve_for_backend(true)
            .expect("explicit scalar selection must be legal everywhere"),
        BatchHitWriter::Scalar
    );
}
