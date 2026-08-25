//! Contracts for CUDA resident readback fusion plumbing.

mod common;
use common::resident_dispatch_source;

#[test]
fn compiled_resident_single_readbacks_use_fused_copy_path() {
    let source = include_str!("../src/backend/resident_io.rs");
    let method = source
        .split("pub(crate) fn download_resident_readbacks_many_into")
        .nth(1)
        .and_then(|tail| tail.split("fn download_resident_copies_many_into").next())
        .expect("Fix: resident_io.rs must expose download_resident_readbacks_many_into before the copy helpers.");

    assert!(
        method.contains("self.download_resident_fused_copies_many_into(&copies, outputs)")
            && !method.contains("self.download_resident_copies_many_into("),
        "Fix: compiled resident readbacks must use fused D2H copies instead of one transfer per requested output range."
    );
}

#[test]
fn compiled_resident_batch_readbacks_use_fused_copy_path() {
    let source = include_str!("../src/backend/resident_io.rs");
    let method = source
        .split("pub(crate) fn download_resident_readback_batches_many_into")
        .nth(1)
        .and_then(|tail| tail.split("pub fn free_resident").next())
        .expect("Fix: resident_io.rs must expose download_resident_readback_batches_many_into before free_resident.");

    assert!(
        method.contains("self.download_resident_fused_copy_batches_many_into(")
            && !method.contains("host_transfers.collect_output_into(transfer_index, output)"),
        "Fix: resident batch readbacks must flatten and fuse D2H copies before slicing outputs back into batch slots."
    );
}

#[test]
fn compiled_resident_batch_readback_materialization_has_no_direct_view_indexing() {
    let source = include_str!("../src/backend/resident_io.rs");
    let helper = source
        .split("fn download_resident_fused_copy_batches_many_into")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) fn download_resident_readback_batches_many_into").next())
        .expect("Fix: resident_io.rs must expose fused batch readback materialization before public batch readback validation.");

    assert!(
        helper.contains("let mut fused_views = fused_readbacks.views.iter().copied();")
            && helper.contains("fused_views.next().ok_or_else(|| BackendError::InvalidProgram")
            && helper.contains("fused_views.next().is_some()")
            && !helper.contains("fused_readbacks.views[transfer_index]"),
        "Fix: CUDA fused batch readback materialization must turn view-count drift into typed BackendError instead of direct indexing."
    );
}

#[test]
fn resident_readback_fusion_is_scoped_by_handle_identity() {
    let cuda_source = include_str!("../src/backend/resident_readback_fusion.rs");
    let driver_source = include_str!("../../vyre-driver/src/resident_transfer_fusion.rs");
    let helper = cuda_source
        .split("fn fuse_resident_readback_copies")
        .nth(1)
        .expect("Fix: resident_readback_fusion.rs must expose fuse_resident_readback_copies.");

    assert!(
        helper.contains("fuse_resident_transfer_intervals(requested)")
            && cuda_source.contains("type ResidentReadbackCopy = ResidentTransferInterval")
            && driver_source.contains("iter_is_monotonic_by_key(")
            && driver_source.contains(
                "sort_by_key_if_needed(&mut ordered, |(_, copy)| (copy.handle_id, copy.src))"
            )
            && driver_source.contains("last.handle_id == copy.handle_id && copy.src <= last_end"),
        "Fix: resident readback fusion must delegate to the backend-neutral interval helper and merge only overlapping/adjacent ranges from the same resident handle."
    );
}

#[test]
fn resident_sequence_readbacks_share_the_common_fusion_helper() {
    let source = resident_dispatch_source();
    let method = source
        .split("pub(crate) fn fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into")
        .nth(1)
        .and_then(|tail| tail.split("pub fn dispatch_resident_timed").next())
        .expect("Fix: resident_dispatch.rs must expose fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into before dispatch_resident_timed.");

    assert!(
        method.contains("fuse_resident_readback_copies(&requested_readbacks)")
            && !method.contains("readback_order.sort_unstable_by_key")
            && !method.contains("struct ReadbackOutputView"),
        "Fix: resident sequence readbacks must reuse the shared fusion helper instead of carrying a nested duplicate interval-merger."
    );
}

#[test]
fn fused_resident_readbacks_validate_views_before_materializing_outputs() {
    let resident_io = include_str!("../src/backend/resident_io.rs");
    let sequence_source = resident_dispatch_source();
    let fusion_source = include_str!("../src/backend/resident_readback_fusion.rs");

    assert!(
        fusion_source.contains("pub(crate) fn validate_fused_resident_readbacks(")
            && fusion_source.contains("fused.views.len() != requested_output_slots")
            && fusion_source.contains("staged_copy_bytes")
            && fusion_source.contains("fused.bytes != staged_copy_bytes")
            && fusion_source.contains("view_end > copy.byte_len"),
        "Fix: CUDA resident readback fusion must expose one validator for output-view cardinality, telemetry byte accounting, and bounds."
    );
    assert_eq!(
        resident_io
            .matches("validate_fused_resident_readbacks(\n            &fused_readbacks")
            .count(),
        3,
        "Fix: CUDA resident IO must validate fused readback views before every single and batched output materialization path."
    );
    assert_eq!(
        sequence_source
            .matches("validate_fused_resident_readbacks(\n                &fused_readbacks")
            .count(),
        1,
        "Fix: CUDA resident sequence readbacks must validate fused views before zip-based output materialization."
    );
    let staging_helper = resident_io
        .split("fn stage_fused_resident_readbacks_to_host")
        .nth(1)
        .expect("Fix: resident_io.rs must keep fused resident readback staging centralized.")
        .split("fn record_resident_readback_telemetry")
        .next()
        .expect("Fix: resident readback staging must precede telemetry recording.");
    assert!(
        staging_helper.contains("fused_readbacks.non_empty_copy_count")
            && staging_helper.contains("fused_readbacks.copies.len()")
            && !staging_helper.contains("requested_output_slots"),
        "Fix: CUDA resident readback staging must reserve host transfer slots by fused copy count, not requested output slots."
    );
}

#[test]
fn fused_resident_readback_materialization_preflights_output_storage_before_copying() {
    let source = include_str!("../src/backend/resident_io.rs");
    let sequence_source = resident_dispatch_source();
    assert!(
        source.contains("fn reserve_borrowed_resident_readback_outputs(")
            && source.contains("fn reserve_resident_readback_outputs(")
            && source.contains("fn reserve_resident_readback_batch_outputs("),
        "Fix: CUDA resident readback materialization must preflight caller output storage through explicit helpers before copying bytes."
    );

    let borrowed = source
        .split("pub fn download_resident_ranges_into")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn download_resident_readbacks_many")
                .next()
        })
        .expect("Fix: ranged resident download must precede compiled readback helpers.");
    let borrowed_reserve = borrowed
        .find("reserve_borrowed_resident_readback_outputs(&fused_readbacks.views, outputs)?")
        .expect("Fix: borrowed ranged readback must reserve every caller output before materialization.");
    let borrowed_collect = borrowed
        .find("host_transfers.collect_output_range_into")
        .expect("Fix: borrowed ranged readback must still collect from staged host transfers.");
    assert!(
        borrowed_reserve < borrowed_collect,
        "Fix: borrowed ranged resident readback must reserve destination bytes before the first collect_output_range_into call."
    );

    let sequence = sequence_source
        .split(
            "pub(crate) fn fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into",
        )
        .nth(1)
        .and_then(|tail| tail.split("pub fn dispatch_resident_timed").next())
        .expect(
            "Fix: resident sequence readback implementation must precede dispatch_resident_timed.",
        );
    let sequence_reserve = sequence
        .find("reserve_borrowed_resident_readback_outputs(&fused_readbacks.views, outputs)?")
        .expect("Fix: resident sequence fused readback must reserve every caller output before staging or materialization.");
    let sequence_stage = sequence
        .find("readback_host_transfers = Some(HostTransferAllocations::with_capacity")
        .expect("Fix: resident sequence fused readback must still stage fused host transfers.");
    assert!(
        sequence.contains("std::mem::forget(readback_host_transfers)"),
        "Fix: resident sequence fused readback must retain host transfer ownership for unproven-completion cleanup."
    );
    let sequence_collect = sequence
        .find("transfers.collect_output_range_into")
        .expect("Fix: resident sequence fused readback must still collect staged ranges.");
    assert!(
        sequence_reserve < sequence_stage && sequence_reserve < sequence_collect,
        "Fix: resident sequence fused readback must preflight destination bytes before host staging and collection."
    );

    let single = source
        .split("fn download_resident_fused_copies_many_into")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn download_resident_fused_copy_batches_many_into")
                .next()
        })
        .expect("Fix: single fused readback helper must precede batched fused helper.");
    let single_reserve = single
        .find("reserve_resident_readback_outputs(&fused_readbacks.views, outputs)?")
        .expect("Fix: single fused readback helper must reserve output storage before staging/collection.");
    let single_collect = single
        .find("host_transfers.collect_output_range_into")
        .expect("Fix: single fused readback helper must still collect staged ranges.");
    assert!(
        single_reserve < single_collect && !single.contains("resize_vec_slots(outputs"),
        "Fix: single fused resident readback must preflight output slots and bytes before copying instead of resizing during collection."
    );

    let batch = source
        .split("fn download_resident_fused_copy_batches_many_into")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn download_resident_readback_batches_many_into")
                .next()
        })
        .expect(
            "Fix: batched fused readback helper must precede public batch readback validation.",
        );
    let batch_reserve = batch
        .find("reserve_resident_readback_batch_outputs(")
        .expect("Fix: batched fused readback helper must reserve nested output storage before staging/collection.");
    let batch_collect = batch
        .find("host_transfers.collect_output_range_into")
        .expect("Fix: batched fused readback helper must still collect staged ranges.");
    assert!(
        batch_reserve < batch_collect && !batch.contains("resize_vec_slots(outputs"),
        "Fix: batched fused resident readback must preflight nested output slots and bytes before copying instead of resizing during collection."
    );

    let reserve_helper = source
        .split("fn reserve_resident_readback_outputs")
        .nth(1)
        .and_then(|tail| tail.split("fn next_resident_readback_view").next())
        .expect("Fix: single reserve helper must precede the batched view cursor.");
    assert!(
        reserve_helper.contains("let mut appended_outputs = reserved_vec(")
            && reserve_helper.contains("outputs.truncate(views.len())")
            && reserve_helper.contains("outputs.extend(appended_outputs)"),
        "Fix: single resident readback reserve helper must build appended output slots before mutating caller output length."
    );
}
